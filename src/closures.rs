use std::mem;

use crate::{
    functions::FunctionHandle,
    handles::Handles,
    heap::{Collector, Handle, Pool, Traceable, CLOSURE, FUNCTION},
    upvalues::UpvalueHandle,
};

pub type ClosureHandle = Handle<CLOSURE>;

pub struct Closures {
    functions: Vec<FunctionHandle>,
    handles: Handles,
    offsets: Vec<u32>,
    upvalue_counts: Vec<u8>,
    pub upvalues: Box<[UpvalueHandle]>,
    next: usize,
}

impl Closures {
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
            handles: Handles::new(),
            offsets: Vec::new(),
            upvalue_counts: Vec::new(),
            upvalues: vec![Handle(0); 8].into_boxed_slice(),
            next: 0,
        }
    }

    const TOP_BIT: u32 = 0x8000_0000;

    pub fn get_function(&self, ch: ClosureHandle) -> FunctionHandle {
        if ch.0 & Self::TOP_BIT == 0 {
            return Handle(ch.0);
        }
        let index = (ch.0 ^ Self::TOP_BIT) as usize;
        self.functions[index]
    }

    // remember that offsets can be out of order
    pub fn up(&self, ch: ClosureHandle) -> usize {
        // assert_ne!(ch.0 & Self::TOP_BIT, 0);
        if ch.0 & Self::TOP_BIT == 0 {
            return 0;
        }
        let h = (ch.0 ^ Self::TOP_BIT) as usize;
        self.offsets[h] as usize
    }

    pub fn upvalues_ref(&mut self, ch: ClosureHandle) -> &[UpvalueHandle] {
        let j = self.up(ch);
        &self.upvalues[j..]
    }

    pub fn upvalues_mut(&mut self, ch: ClosureHandle) -> &mut [UpvalueHandle] {
        let j = self.up(ch);
        &mut self.upvalues[j..]
    }

    fn grow(&mut self) {
        let capacity = self.upvalues.len() * 2;
        let old = mem::replace(
            &mut self.upvalues,
            vec![Handle(0); capacity].into_boxed_slice(),
        );
        let mut next = 0;
        for i in 0..self.functions.len() {
            if !self.handles.is_marked(i as u32) {
                continue;
            }
            let offset = self.offsets[i] as usize;
            self.offsets[i] = next as u32;
            for j in 0..self.upvalue_counts[i] as usize {
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

        let index = self.handles.next() as usize;
        while self.functions.len() <= index {
            self.functions.push(Handle(u32::MAX));
            self.offsets.push(0);
            self.upvalue_counts.push(0);
        }
        self.functions[index] = fh;
        self.offsets[index] = self.next as u32;
        self.upvalue_counts[index] = uc as u8;
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
            + self.functions.capacity() * 9
            + self.upvalues.len() * 4
    }
    fn trace(&mut self, collector: &mut Collector) {
        let mut marked = Vec::new();
        while let Some(handle) = collector.handles[CLOSURE].pop() {
            if handle & Self::TOP_BIT == 0 {
                collector.push(FUNCTION, handle);
                continue;
            }
            let handle = handle ^ Self::TOP_BIT;
            if self.handles.mark(handle) {
                marked.push(handle);
            }
        }
        for &index in &marked {
            self.functions[index as usize].trace(collector);
            let index = index as usize;
            let from = self.offsets[index] as usize;
            let to = from + self.upvalue_counts[index] as usize;
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
        let sum = closures.functions.len();
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

        // add handle to collector
        closure.trace(&mut collector);
        closures.trace(&mut collector);
        // how!?
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
