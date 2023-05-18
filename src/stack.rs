use std::{mem::MaybeUninit};

use crate::{
    class::{Method, Path},
    object::{Handle, RuntimeError, Upvalue, Value},
};

pub struct Stack<T: Copy, const SIZE: usize> {
    entries: [T; SIZE],
    count: usize,
}

// maybe different types require different implementations dispite the superficial similarities
impl<T: Copy, const SIZE: usize> Stack<T, SIZE> {
    pub fn new(default: T) -> Self {
        let entries = unsafe {
            let mut entries: [MaybeUninit<T>; SIZE] = MaybeUninit::uninit().assume_init();
            for entry in entries.iter_mut() {
                *entry = MaybeUninit::new(default)
            }
            std::mem::transmute_copy::<_, [T; SIZE]>(&entries)
        };
        Self { entries, count: 0 }
    }
    pub fn reset(&mut self) {
        self.count = 0;
    }
    pub fn push(&mut self, entry: T) -> Result<usize, RuntimeError> {
        if self.count == SIZE {
            return Err(RuntimeError::StackOverflow);
        }
        self.entries[self.count] = entry;
        self.count += 1; // no check?
        Ok(self.count)
    }
    pub fn pop(&mut self) -> Option<T> {
        if self.count == 0 {
            return None;
        }
        self.count -= 1; // no check?
        Some(self.entries[self.count])
    }
    pub fn peek(&mut self, distance: usize) -> Option<&mut T> {
        let index = self.count - 1 - distance;
        if index <= 0 || index >= SIZE {
            return None;
        }
        Some(&mut self.entries[self.count - 1 - distance])
    }
}

const MAX_FRAMES: usize = 1 << 6;
const U8_COUNT: usize = 1 << 8;
const STACK_SIZE: usize = MAX_FRAMES * U8_COUNT;

#[derive(Copy, Clone)]
struct CallFrame {
    ip: usize,
    slots: usize,
    method: Path<Method>, // what is needed, and how do we do it?
}

pub struct VMStacks {
    values: Stack<Value, STACK_SIZE>,
    frames: Stack<CallFrame, MAX_FRAMES>,
    open_upvalues: Stack<Handle<Upvalue>, U8_COUNT>,
}
