use std::mem;

use crate::{
    functions::FunctionHandle,
    handles::{Column, HandleSet},
    heap::{Collector, Handle, Pool, Traceable, CLOSURE, FUNCTION},
    upvalues::UpvalueHandle,
};

pub type ClosureHandle = Handle<CLOSURE>;

pub struct Closures {
    functions: Column<FunctionHandle>,
    handles: HandleSet,
    next: usize,
    offsets: Column<u32>,
    pub upvalues: Box<[UpvalueHandle]>,
    upvalue_counts: Column<u8>,
}

impl Closures {
    pub fn new() -> Self {
        Self {
            functions: Column::new(),
            handles: HandleSet::new(),
            next: 0,
            offsets: Column::new(),
            upvalue_counts: Column::new(),
            upvalues: vec![Handle(0); 8].into_boxed_slice(),
        }
    }

    const TOP_BIT: u32 = 0x8000_0000;

    pub fn get_function(&self, ch: ClosureHandle) -> FunctionHandle {
        if ch.0 & Self::TOP_BIT == 0 {
            return Handle(ch.0);
        }
        self.functions.get(ch.0 ^ Self::TOP_BIT)
    }

    // remember that offsets can be out of order
    pub fn up(&self, ch: ClosureHandle) -> usize {
        // assert_ne!(ch.0 & Self::TOP_BIT, 0);
        if ch.0 & Self::TOP_BIT == 0 {
            return 0;
        }
        self.offsets.get(ch.0 ^ Self::TOP_BIT) as usize
    }

    pub fn set_upvalue(&mut self, ch: ClosureHandle, index: usize, uh: UpvalueHandle) {
        self.upvalues[self.up(ch) + index] = uh;
    }

    fn grow(&mut self) {
        let capacity = self.upvalues.len() * 2;
        let old = mem::replace(
            &mut self.upvalues,
            vec![Handle(0); capacity].into_boxed_slice(),
        );
        let mut next = 0;
        for i in 0..self.functions.values.len() {
            let i = i as u32;
            if !self.handles.is_marked(i) {
                continue;
            }
            let offset = self.offsets.get(i) as usize;
            self.offsets.set(i, next as u32);
            for j in 0..self.upvalue_counts.get(i) as usize {
                self.upvalues[next] = old[offset + j];
                next += 1;
            }
        }
        self.next = next;
    }

    pub fn new_closure(&mut self, fh: FunctionHandle, uc: usize) -> ClosureHandle {
        if uc == 0 {
            return ClosureHandle::from(fh.0);
        }
        let index = self.handles.next();
        self.functions.set(index, fh);
        self.offsets.set(index, self.next as u32);
        self.upvalue_counts.set(index, uc as u8);
        self.next += uc;
        if self.next >= self.upvalues.len() {
            self.grow();
        }
        ClosureHandle::from(index as u32 ^ Self::TOP_BIT)
    }
}

const BYTE_COUNT: usize = mem::size_of::<Closures>();

impl Pool<CLOSURE> for Closures {
    fn sweep(&mut self) {}
    fn byte_count(&self) -> usize {
        BYTE_COUNT
            + self.handles.byte_count()
            + self.functions.byte_count()
            + self.offsets.byte_count()
            + self.upvalue_counts.byte_count()
            + self.upvalues.len() * 4
    }
    fn mark(&mut self, handle: u32) -> bool {
        // cannot tell if functions are already marked.
        handle & Self::TOP_BIT == 0 || self.handles.mark(handle ^ Self::TOP_BIT)
    }
    fn trace_all(&mut self, marked: &Vec<u32>, collector: &mut Collector) {
        for &handle in marked {
            if handle & Self::TOP_BIT == 0 {
                collector.push(FUNCTION, handle);
                continue;
            }
            let index = handle ^ Self::TOP_BIT;
            self.functions.get(index).trace(collector);
            let from = self.offsets.get(index) as usize;
            let to = from + self.upvalue_counts.get(index) as usize;
            for j in from..to {
                self.upvalues[j].trace(collector);
            }
        }
    }
    fn reset(&mut self) {
        self.handles.clear();
    }
}

#[cfg(test)]
mod tests {

    use crate::heap::{FUNCTION, UPVALUE};

    use super::*;

    #[test]
    pub fn make_new() {
        let mut closures = Closures::new();
        // try one
        let closure = closures.new_closure(Handle::from(2), 2);
        assert_eq!(closures.get_function(closure).index(), 2);
        closures.upvalues[closures.up(closure) + 1] = Handle::from(135);

        // try another
        let closure2 = closures.new_closure(Handle::from(4), 3);
        assert_eq!(closures.get_function(closure2).index(), 4);
        closures.upvalues[closures.up(closure2) + 2] = Handle::from(135);

        // try an empty one
        let closure3 = closures.new_closure(Handle::from(6), 0);
        assert_eq!(closures.get_function(closure3).index(), 6);
        let sum = closures.functions.values.len();
        assert_eq!(sum, 2);
    }

    #[test]
    pub fn tracing() {
        let mut closures = Closures::new();
        let closure = closures.new_closure(Handle::from(2), 2);
        assert_eq!(closure.0, Closures::TOP_BIT);
        closures.upvalues[closures.up(closure) + 1] = Handle::from(135);

        let mut collector = Collector::new();

        // remove marks, like in a real mark and sweep
        closures.reset();
        closures.trace_all(&vec![Closures::TOP_BIT], &mut collector);
        assert_eq!(collector.handles[FUNCTION], vec![2]);
        assert_eq!(collector.handles[UPVALUE], vec![0, 135]);
    }

    #[test]
    pub fn sweeping() {
        let mut closures = Closures::new();
        let closure = closures.new_closure(Handle::from(2), 2);
        closures.reset();
        closures.sweep();
        let closure2 = closures.new_closure(Handle::from(2), 2);

        assert_eq!(closure, closure2);
    }
}
