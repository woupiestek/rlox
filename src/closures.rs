use crate::{
    bitarray::BitArray,
    functions::FunctionHandle,
    heap::{Collector, Handle, Pool, CLOSURE},
    upvalues::UpvalueHandle,
};

pub type ClosureHandle = Handle<CLOSURE>;

impl Default for ClosureHandle {
    fn default() -> Self {
        Self(Default::default())
    }
}

const SHIFT: u8 = 24;
const MASK: usize = 0xffffff;
const COUNTS: usize = 255;

pub struct Closures {
    free: [u32; COUNTS],
    functions: Vec<Vec<u32>>,
    upvalues: Vec<Vec<UpvalueHandle>>,
}

impl Closures {
    pub fn new() -> Self {
        Self {
            free: [0; COUNTS],
            functions: Vec::new(),
            upvalues: Vec::new(),
        }
    }

    fn upvalue_count(ch: ClosureHandle) -> usize {
        ch.index() >> SHIFT
    }

    pub fn get_function(&self, ch: ClosureHandle) -> FunctionHandle {
        let uc = Closures::upvalue_count(ch);
        FunctionHandle::from(if uc == 0 {
            // risk if functions handles get higher than 0xffffff
            ch.0
        } else {
            self.functions[uc - 1][ch.index() & MASK]
        })
    }

    pub fn get_upvalue(&self, ch: ClosureHandle, i: usize) -> UpvalueHandle {
        let uc = Closures::upvalue_count(ch);
        assert_ne!(uc, 0);
        self.upvalues[uc - 1][uc * (ch.index() & MASK) + i]
    }

    pub fn set_upvalue(&mut self, ch: ClosureHandle, i: usize, uh: UpvalueHandle) {
        let uc = Closures::upvalue_count(ch);
        assert_ne!(uc, 0);
        self.upvalues[uc - 1][uc * (ch.index() & MASK) + i] = uh;
    }

    // simplify offsets for now
    fn force_offset(&mut self, uc: usize) {
        while self.functions.len() < uc {
            self.functions.push(Vec::new());
        }
        while self.upvalues.len() < uc {
            self.upvalues.push(Vec::new());
        }
    }

    pub fn new_closure(&mut self, fh: FunctionHandle, uc: usize) -> ClosureHandle {
        if uc == 0 {
            return ClosureHandle::from(fh.0);
        }
        let free = self.free[uc - 1] as usize;
        if free > MASK {
            panic!("Out of closure space")
        }
        self.force_offset(uc);
        let functions = &mut self.functions[uc - 1];
        if free < functions.len() {
            // reuse memory
            self.free[uc - 1] = functions[free];
            functions[free] = fh.0;
        } else {
            self.free[uc - 1] += 1;
            functions.push(fh.0);
            for _ in 0..uc {
                // push placeholders
                self.upvalues[uc - 1].push(UpvalueHandle::from(0));
            }
        }
        ClosureHandle::from((uc << SHIFT) as u32 + free as u32)
    }
}

impl Pool<CLOSURE> for Closures {
    fn byte_count(&self) -> usize {
        let mut capacity = 0;
        for vec in &self.functions {
            capacity += vec.capacity()
        }
        for vec in &self.upvalues {
            capacity += vec.capacity()
        }
        4 * capacity
    }
    fn trace(&self, handle: Handle<CLOSURE>, collector: &mut Collector) {
        let uc = Closures::upvalue_count(handle);
        if uc == 0 {
            collector.push(FunctionHandle::from(handle.0));
            return;
        }
        let index = handle.index() & MASK;
        collector.push(FunctionHandle::from(self.functions[uc - 1][index]));
        for i in 0..uc {
            collector.push(UpvalueHandle::from(self.upvalues[uc - 1][uc * index + i]));
        }
    }
    fn sweep(&mut self, marks: &BitArray) {
        for i in 0..self.functions.len() {
            let functions = &mut self.functions[i];
            self.free[i] = functions.len() as u32;
            for j in 0..functions.len() {
                if !marks.has(((i + 1) << SHIFT) + j) {
                    functions[j] = self.free[i];
                    self.free[i] = j as u32;
                }
            }
        }
    }
    fn count(&self) -> usize {
        let mut count = 0;
        for functions in &self.functions {
            count += functions.len();
        }
        count
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
        closures.set_upvalue(closure, 1, Handle::from(135));
        assert_eq!(closures.get_upvalue(closure, 1).index(), 135);

        // try another
        let closure2 = closures.new_closure(Handle::from(4), 3);
        assert_eq!(closures.get_function(closure2).index(), 4);
        closures.set_upvalue(closure2, 2, Handle::from(135));
        assert_eq!(closures.get_upvalue(closure2, 2).index(), 135);

        // try an empty one
        let closure3 = closures.new_closure(Handle::from(6), 0);
        assert_eq!(closures.get_function(closure3).index(), 6);
        assert_eq!(closures.count(), 2);
    }

    #[test]
    pub fn tracing() {
        let mut closures = Closures::new();
        let closure = closures.new_closure(Handle::from(2), 2);
        closures.set_upvalue(closure, 1, Handle::from(135));

        let mut collector = Collector::new();
        closures.trace(closure, &mut collector);
        assert_eq!(collector.handles[FUNCTION], vec![2]);
        assert_eq!(collector.handles[UPVALUE], vec![0, 135]);
    }

    #[test]
    pub fn sweeping() {
        let mut closures = Closures::new();
        let closure = closures.new_closure(Handle::from(2), 2);
        closures.sweep(&BitArray::new());
        let closure2 = closures.new_closure(Handle::from(2), 2);

        assert_eq!(closure, closure2);
    }
}
