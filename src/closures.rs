use crate::{
    bitarray::BitArray,
    functions::{FunctionHandle, Functions},
    heap::{Collector, Handle, Pool, CLOSURE},
    u32s::U32s,
    upvalues::UpvalueHandle,
};

pub type ClosureHandle = Handle<CLOSURE>;

impl Default for ClosureHandle {
    fn default() -> Self {
        Self(Default::default())
    }
}

pub struct Closures {
    functions: U32s,
    upvalues: Vec<Option<Box<[UpvalueHandle]>>>,
    // to get a byte count
    upvalue_count: usize,
    place_holder: UpvalueHandle,
}

impl Closures {
    pub fn new() -> Self {
        Self {
            functions: U32s::new(),
            upvalues: Vec::new(),
            upvalue_count: 0,
            place_holder: UpvalueHandle::from(0),
        }
    }

    pub fn get_function(&self, fh: ClosureHandle) -> FunctionHandle {
        FunctionHandle::from(self.functions.get(fh.0))
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
        let i = self.functions.store(fh.0);
        while self.upvalues.len() < self.functions.count() {
            self.upvalues.push(None)
        }
        self.upvalues[i as usize] = upvalues;
        ClosureHandle::from(i)
    }
}

impl Pool<CLOSURE> for Closures {
    fn byte_count(&self) -> usize {
        4 * self.functions.capacity() + 4 * self.upvalue_count + 8 * self.upvalues.capacity()
    }
    fn trace(&self, handle: Handle<CLOSURE>, collector: &mut Collector) {
        collector.push(self.get_function(handle));
        if let Some(upvalues) = &self.upvalues[handle.index()] {
            for i in 0..upvalues.len() {
                collector.push(upvalues[i])
            }
        }
    }
    fn sweep(&mut self, marks: &BitArray) {
        self.functions.sweep(marks);
        for i in self.functions.free_indices() {
            let option = self.upvalues[i as usize].take();
            if let Some(upvalues) = option {
                self.upvalue_count -= upvalues.len()
            }
        }
    }

    fn count(&self) -> usize {
        self.functions.count()
    }
}
