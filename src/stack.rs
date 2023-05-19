use std::{
    alloc::{self, Layout},
    mem,
    ptr::{self},
};

use crate::object::RuntimeError;

pub struct Stack<T, const CAPACITY: usize> {
    entries: *mut T,
    len: usize,
}

// maybe different types require different implementations dispite the superficial similarities
impl<T: Copy, const CAPACITY: usize> Stack<T, CAPACITY> {
    pub fn new() -> Self {
        assert!(mem::size_of::<T>() != 0, "We're not ready to handle ZSTs");
        let layout = Layout::array::<T>(CAPACITY).unwrap();
        let ptr = unsafe { alloc::alloc(layout) };
        if ptr.is_null() {
            alloc::handle_alloc_error(layout);
        }
        Self {
            entries: ptr as *mut T,
            len: 0,
        }
    }

    pub fn push(&mut self, entry: T) -> Result<usize, RuntimeError> {
        if self.len == CAPACITY {
            return Err(RuntimeError::StackOverflow);
        }
        unsafe { ptr::write({ self.entries.add(self.len) }, entry) }
        self.len += 1;
        Ok(self.len)
    }
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        } else {
            self.len -= 1;
            unsafe { Some(ptr::read({ self.entries.add(self.len) })) }
        }
    }
    pub fn peek(&mut self, distance: usize) -> Option<&mut T> {
        if distance > self.len {
            None
        } else {
            // god I hope this is right
            unsafe { Some(&mut *({ self.entries.add(self.len - 1 - distance) })) }
        }
    }
    pub fn reset(&mut self) {
        for i in 0..self.len {
            unsafe {
                ptr::drop_in_place({
                    let ref this = self;
                    this.entries.add(i)
                });
            }
        }
        self.len = 0;
    }
}
