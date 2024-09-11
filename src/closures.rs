use crate::{
    bitarray::BitArray,
    functions::{FunctionHandle, Functions},
    heap::{Collector, Handle, Pool, CLOSURE},
    u32s::U32s,
    upvalues::UpvalueHandle,
};

pub type ClosureHandle = Handle<CLOSURE>;

// use handles >= UNARY_TAG for nullary functions
const UNARY_TAG: u32 = 0x8000_0000;

impl Default for ClosureHandle {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl ClosureHandle {
    fn is_nullary(&self) -> bool {
        self.0 >= UNARY_TAG
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
        if fh.is_nullary() {
            return FunctionHandle::from(UNARY_TAG ^ fh.0);
        }
        FunctionHandle::from(self.functions.get(fh.0))
    }

    pub fn get_upvalue(&self, fh: ClosureHandle, i: usize) -> UpvalueHandle {
        assert!(!fh.is_nullary());
        self.upvalues[fh.index()].as_ref().unwrap()[i]
    }

    pub fn set_upvalue(&mut self, fh: ClosureHandle, i: usize, uh: UpvalueHandle) {
        assert!(!fh.is_nullary());
        self.upvalues[fh.index()].as_mut().unwrap()[i] = uh;
    }

    pub fn new_closure(&mut self, fh: FunctionHandle, functions: &Functions) -> ClosureHandle {
        let upvalue_count = functions.upvalue_count(fh);
        if upvalue_count == 0 && fh.0 < UNARY_TAG {
            return ClosureHandle::from(UNARY_TAG | fh.0);
        }
        let upvalues = if upvalue_count == 0 {
            None
        } else {
            self.upvalue_count += upvalue_count;
            Some(vec![self.place_holder; upvalue_count].into_boxed_slice())
        };
        let i = self.functions.store(fh.0);
        assert!(i < UNARY_TAG, "closure pool ran out of space");
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
        if !handle.is_nullary() {
            if let Some(upvalues) = &self.upvalues[handle.index()] {
                for i in 0..upvalues.len() {
                    collector.push(upvalues[i])
                }
            }
        }
    }
    fn sweep(&mut self, marks: &BitArray) {
        self.functions.sweep(marks);
        for i in self.functions.free_indices() {
            if let Some(upvalues) = self.upvalues[i as usize].take() {
                self.upvalue_count -= upvalues.len()
            }
        }
    }
    fn mark(&self, collector: &mut Collector) -> bool {
        if collector.handles[CLOSURE].is_empty() {
            return true;
        }
        while let Some(i) = collector.handles[CLOSURE].pop() {
            if i >= UNARY_TAG {
                // do not mark, nothing was stored!
                collector.push(self.get_function(Handle::from(i)));
                continue;
            }
            if !collector.marks[CLOSURE].has(i as usize) {
                collector.marks[CLOSURE].add(i as usize);
                self.trace(Handle::from(i), collector);
            }
        }
        false
    }

    fn count(&self) -> usize {
        self.functions.count()
    }
}
