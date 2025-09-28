// idea
// we are keeping the bit set in the garbage collector anyway, so why not build handles on top?

use std::mem;

// a virtual set of handles for objects
// represented by a bitset
// subdivided in buckets of 8, since that is the size of a byte.
pub struct Handles {
    count: usize,
    index: usize,
    marked: Box<[u8]>,
}

const THREE_BIT_MASK: u32 = 7;

impl Handles {
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

    // pub fn unmark(&mut self, h: u32) -> bool {
    //     let i = (h >> 3) as usize;
    //     if self.marked.len() <= i {
    //         return false;
    //     }
    //     let j = 1 << (h & THREE_BIT_MASK);
    //     if self.marked[i] & j == 1 {
    //         self.count -= 1;
    //         self.marked[i] &= !j;
    //         if self.index > i {
    //             // put first in line to allocate
    //             self.index = i
    //         }
    //         true
    //     } else {
    //         false
    //     }
    // }

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
