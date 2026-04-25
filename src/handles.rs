// idea
// we are keeping the bit set in the garbage collector anyway, so why not build handles on top?

use std::mem;

use crate::heap::{Collector, Handle};

// a virtual set of handles for objects
// represented by a bitset
// subdivided in buckets of 8, since that is the size of a byte.
pub struct HandleSet {
    count: usize,
    index: usize,
    marked: Vec<u8>,
}

const THREE_BIT_MASK: u32 = 7;

impl HandleSet {
    // start at 64 bit
    pub fn new() -> Self {
        Self {
            index: 0,
            count: 0,
            marked: Vec::new(),
        }
    }

    fn grow(&mut self, i: usize) {
        self.marked.resize(i + 1, 0);
    }

    pub fn clear(&mut self) {
        self.count = 0;
        self.index = 0;
        self.marked.fill(0);
    }

    pub fn is_marked(&self, h: u32) -> bool {
        let i = (h >> 3) as usize;
        let j = 1 << (h & THREE_BIT_MASK);
        i < self.marked.len() && self.marked[i] & j > 0
    }

    pub fn mark(&mut self, h: u32) -> bool {
        let i = (h >> 3) as usize;
        if self.marked.len() <= i {
            self.grow(i);
        }
        let j = 1 << (h & THREE_BIT_MASK);
        if self.marked[i] & j == 0 {
            self.count += 1;
            self.marked[i] |= j;
            true
        } else {
            false
        }
    }

    // look for a free spot
    // start in the 'current bucket'
    // move up by one every time
    // allocate more buckets
    pub fn next(&mut self) -> u32 {
        self.count += 1;
        while self.index < self.marked.len() {
            let j = self.marked[self.index].trailing_ones();
            if j < 8 {
                self.marked[self.index] |= 1 << j;
                return (self.index << 3) as u32 | j;
            }
            self.index += 1;
        }
        self.grow(self.index);
        self.marked[self.index] = 1;
        (self.index << 3) as u32
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn byte_count(&self) -> usize {
        mem::size_of::<Self>() + self.marked.capacity()
    }

    pub fn len(&self) -> usize {
        self.marked.len() << 3
    }
}

pub struct Column<T: Clone + Default> {
    pub values: Vec<T>,
}

impl<T: Clone + Default> Column<T> {
    pub fn new() -> Self {
        Self { values: Vec::new() }
    }
    pub fn set(&mut self, index: u32, value: T) {
        let index = index as usize;
        if self.values.len() <= index {
            self.values
                .resize((index + 1).next_power_of_two(), Default::default());
        }
        self.values[index] = value;
    }
    pub fn get(&self, index: u32) -> T {
        let index = index as usize;
        if index >= self.values.len() {
            Default::default()
        } else {
            self.values[index as usize].clone()
        }
    }
    pub fn byte_count(&self) -> usize {
        mem::size_of::<Self>() + self.values.capacity() * mem::size_of::<T>()
    }
}

impl<const KIND: usize> Column<Handle<KIND>> {
    pub fn trace_all(&self, marked: &Vec<u32>, collector: &mut Collector) {
        for &i in marked {
            collector.handles[KIND].push(self.values[i as usize].0)
        }
    }
}
