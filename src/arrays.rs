use std::{
    mem,
    ops::{Index, IndexMut, Range},
};

/*
* Memory safe free list allocator
*
* Note: the size of the arrays can be greater than requested, to avoid resizing too often.
* The caller is responsible for checking the length and deciding whether to realloc or not.
*/

#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub struct Array {
    from: u32,
    len: u32,
}

impl Array {
    pub fn ptr(&self) -> usize {
        self.from as usize
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn range(&self) -> Range<usize> {
        (self.from as usize)..((self.from + self.len) as usize)
    }
}

pub struct Arrays<A: Copy + Default> {
    elements: Vec<A>,
    free: Vec<Array>,
}

impl<A: Copy + Default> Arrays<A> {
    pub fn new() -> Self {
        Self {
            elements: Vec::with_capacity(8),
            free: Vec::new(),
        }
    }

    fn alloc(&mut self, min_len: usize) -> Array {
        for i in (0..self.free.len()).rev() {
            if self.free[i].len() >= min_len {
                let array = self.free.swap_remove(i);
                for i in array.range() {
                    self.elements[i] = A::default();
                }
                return array;
            }
        }
        let min_len = min_len.max(8);
        let array = Array {
            from: self.elements.len() as u32,
            len: min_len as u32,
        };
        self.elements.append(&mut vec![A::default(); min_len]);
        array
    }

    pub fn free(&mut self, array: Array) {
        if array == Array::default() {
            return;
        }
        self.free.push(array);
    }

    // assume the caller did the len check to decide whether to realloc or not
    pub fn realloc(&mut self, array: Array, new_min_len: usize) -> Array {
        let new_array = self.alloc(new_min_len);
        for i in 0..array.len() {
            self.elements[new_array.ptr() + i] = self.elements[array.ptr() + i];
        }
        self.free(array);
        new_array
    }

    pub fn byte_count(&self) -> usize {
        mem::size_of::<Self>()
            + self.elements.capacity() * mem::size_of::<A>()
            + self.free.capacity() * mem::size_of::<Array>()
    }
}

impl<A: Copy + Default> Index<usize> for Arrays<A> {
    type Output = A;

    fn index(&self, index: usize) -> &Self::Output {
        &self.elements[index]
    }
}

impl<A: Copy + Default> Index<Range<usize>> for Arrays<A> {
    type Output = [A];

    fn index(&self, index: Range<usize>) -> &Self::Output {
        &self.elements[index]
    }
}

impl<A: Copy + Default> IndexMut<usize> for Arrays<A> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.elements[index]
    }
}
