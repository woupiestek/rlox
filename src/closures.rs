use std::u32;

use crate::{
    bitarray::BitArray,
    functions::{FunctionHandle, Functions},
    heap::{Collector, Handle, Pool, CLOSURE},
    upvalues::UpvalueHandle,
};

pub type ClosureHandle = Handle<CLOSURE>;

pub struct Closures {
    functions: Vec<FunctionHandle>,
    upvalues: Vec<Option<Box<[UpvalueHandle]>>>,
    free: Vec<ClosureHandle>,
    // to get a byte count
    upvalue_count: usize,
    place_holder: UpvalueHandle,
}

impl Closures {
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
            upvalues: Vec::new(),
            upvalue_count: 0,
            free: Vec::new(),
            place_holder: UpvalueHandle::from(0),
        }
    }

    pub fn function_handle(&self, fh: ClosureHandle) -> FunctionHandle {
        self.functions[fh.index()]
    }

    pub fn get_upvalue(&self, fh: ClosureHandle, i: usize) -> UpvalueHandle {
        self.upvalues[fh.index()].as_ref().unwrap()[i]
    }

    pub fn set_upvalue(&mut self, fh: ClosureHandle, i: usize, uh: UpvalueHandle) {
        self.upvalues[fh.index()].as_mut().unwrap()[i] = uh;
    }

    pub fn new_closure(&mut self, fh: FunctionHandle, functions: &Functions) -> ClosureHandle {
        let upvalue_count = functions.upvalue_count(fh);
        let upvalues = if upvalue_count > 0 {
            Some(vec![self.place_holder; upvalue_count].into_boxed_slice())
        } else {
            None
        };
        self.upvalue_count += upvalue_count;
        if let Some(i) = self.free.pop() {
            self.functions[i.index()] = fh;
            self.upvalues[i.index()] = upvalues;
            i
        } else {
            let i = self.functions.len() as u32;
            self.functions.push(fh);
            self.upvalues.push(upvalues);
            ClosureHandle::from(i)
        }
    }
}

impl Pool<CLOSURE> for Closures {
    fn byte_count(&self) -> usize {
        // not collecting functions right now
        4 * self.upvalue_count + 8 * self.upvalues.capacity()
    }
    fn trace(&self, handle: Handle<CLOSURE>, collector: &mut Collector) {
        if let Some(upvalues) = &self.upvalues[handle.index()] {
            // not collecting functions right now
            for i in 0..upvalues.len() {
                collector.push(upvalues[i])
            }
        }
    }
    fn sweep(&mut self, marked: &BitArray) {
        self.free.clear();
        for i in 0..self.upvalues.len() {
            if !marked.get(i) {
                if let Some(upvalues) = &self.upvalues[i] {
                    self.upvalue_count -= upvalues.len()
                }
                self.upvalues[i] = None;
                self.free.push(Handle::from(i as u32));
            }
        }
    }

    fn count(&self) -> usize {
        self.functions.len()
    }
}
