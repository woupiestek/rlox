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
    marked: Box<[u8]>,
}

const THREE_BIT_MASK: u32 = 7;

impl HandleSet {
    // start at 64 bit
    pub fn new() -> Self {
        Self {
            index: 0,
            count: 0,
            marked: Box::new([0; 8]),
        }
    }

    fn grow(&mut self) {
        let len = self.marked.len();
        let old = mem::replace(&mut self.marked, vec![0; 2 * len].into_boxed_slice());
        for i in 0..len {
            self.marked[i] = old[i];
        }
    }

    pub fn clear(&mut self) {
        self.count = 0;
        self.index = 0;
        self.marked = vec![0; self.marked.len()].into_boxed_slice();
    }

    pub fn is_marked(&self, h: u32) -> bool {
        let i = (h >> 3) as usize;
        let j = 1 << (h & THREE_BIT_MASK);
        self.marked[i] & j > 0
    }

    // mark taken handles
    pub fn mark(&mut self, h: u32) -> bool {
        let i = (h >> 3) as usize;
        if self.marked.len() <= i {
            self.grow();
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
        while self.index < self.marked.len() {
            let j = self.marked[self.index].trailing_ones();
            if j < 8 {
                self.count += 1;
                self.marked[self.index] |= 1 << j;
                return (self.index << 3) as u32 | j;
            }
            self.index += 1;
        }
        self.grow();
        self.count += 1;
        self.marked[self.index] = 1;
        (self.index << 3) as u32
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn byte_count(&self) -> usize {
        mem::size_of::<Self>() + self.marked.len()
    }
}

// a mapping from a KIND to another, but...
// it could be more useful with a generic type
pub struct Column<T>
where
    T: Clone + Default,
{
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
