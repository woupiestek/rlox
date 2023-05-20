use std::{
    alloc::{self, Layout},
    mem, ptr,
};

pub struct Stack<T> {
    entries: *mut T,
    len: usize,
    cap: usize,
}

// fixed size stack
impl<T> Stack<T> {
    pub fn new(cap: usize) -> Self {
        assert!(cap != 0);
        assert!(mem::size_of::<T>() != 0, "We're not ready to handle ZSTs");
        let layout = Layout::array::<T>(cap).unwrap();
        let ptr = unsafe { alloc::alloc(layout) };
        if ptr.is_null() {
            alloc::handle_alloc_error(layout);
        }
        Self {
            entries: ptr as *mut T,
            len: 0,
            cap,
        }
    }

    pub fn push(&mut self, entry: T) -> Option<usize> {
        if self.len == self.cap {
            return None;
        }
        unsafe { ptr::write(self.entries.add(self.len), entry) }
        self.len += 1;
        Some(self.len)
    }
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        } else {
            self.len -= 1;
            unsafe { Some(ptr::read(self.entries.add(self.len))) }
        }
    }
    pub fn peek(&mut self, distance: usize) -> Option<&mut T> {
        if distance > self.len {
            None
        } else {
            // god I hope this is right
            unsafe { Some(&mut *self.entries.add(self.len - 1 - distance)) }
        }
    }
    pub fn reset(&mut self) {
        for i in 0..self.len {
            unsafe {
                ptr::drop_in_place(self.entries.add(i));
            }
        }
        self.len = 0;
    }
}

impl<T> Drop for Stack<T> {
    fn drop(&mut self) {
        self.reset();
        let layout = Layout::array::<T>(self.cap).unwrap();
        unsafe {
            alloc::dealloc(self.entries as *mut u8, layout);
        }
    }
}
