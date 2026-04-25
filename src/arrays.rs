use std::mem;

/*
* Memory safe free list allocator
*
* Note: the size of the arrays can be greater than requested, to avoid resizing too often.
* The caller is responsible for checking the length and deciding whether to realloc or not.
*/

pub struct Arrays<A: Copy + Default> {
    elements: Vec<A>,
    tos: Vec<u32>,
    free: Vec<u32>,
}

impl<A: Copy + Default> Arrays<A> {
    pub fn new() -> Self {
        Self {
            elements: Vec::with_capacity(8),
            tos: Vec::new(),
            free: Vec::new(),
        }
    }

    pub fn get_ref(&self, index: u32) -> &[A] {
        let index = index as usize;
        let from = if index == 0 { 0 } else { self.tos[index - 1] } as usize;
        let to = self.tos[index] as usize;
        &self.elements[from..to]
    }

    pub fn get_mut(&mut self, index: u32) -> &mut [A] {
        let index = index as usize;
        let from = if index == 0 { 0 } else { self.tos[index - 1] } as usize;
        let to = self.tos[index] as usize;
        &mut self.elements[from..to]
    }

    pub fn len(&self, index: u32) -> usize {
        let index = index as usize;
        let from = if index == 0 { 0 } else { self.tos[index - 1] } as usize;
        let to = self.tos[index] as usize;
        to - from
    }

    pub fn alloc(&mut self, min_len: usize) -> u32 {
        for i in 0..self.free.len() {
            let index = self.free[i];
            if self.len(index) >= min_len {
                let last = self.free.pop().unwrap();
                if i < self.free.len() {
                    self.free[i] = last;
                };
                for i in 0..self.len(index) {
                    self.get_mut(index)[i] = A::default();
                }
                return index;
            }
        }
        // no empty allocations please!
        let min_len = min_len.max(8);
        let to = self.tos.last().unwrap_or(&0) + min_len as u32;
        if to > self.elements.len() as u32 {
            self.elements
                .resize(to.next_power_of_two() as usize, A::default());
        }
        let index = self.tos.len() as u32;
        self.tos.push(to);
        index
    }

    pub fn free(&mut self, index: u32) {
        self.free.push(index);
    }

    // assume the caller did the len check to decide whether to realloc or not
    pub fn realloc(&mut self, index: u32, new_min_len: usize) -> u32 {
        let new_index = self.alloc(new_min_len);
        for i in 0..self.len(index) {
            self.get_mut(new_index)[i] = self.get_ref(index)[i];
        }
        self.free(index);
        new_index
    }

    pub fn byte_count(&self) -> usize {
        mem::size_of::<Self>()
            + self.elements.capacity() * mem::size_of::<A>()
            + self.tos.capacity() * mem::size_of::<u32>()
            + self.free.capacity() * mem::size_of::<u32>()
    }
}
