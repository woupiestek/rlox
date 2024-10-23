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

// limiting to 65536 closures for each of 65536 functions now.
const SHIFT: u8 = 16;
const MASK: usize = 0xffff;

pub struct Closures {
    free: Vec<u32>,
    offsets: Vec<usize>,
    upvalues: Vec<Vec<u32>>,
}

impl Closures {
    pub fn new() -> Self {
        Self {
            free: Vec::new(),
            offsets: vec![0; 1],
            upvalues: Vec::new(),
        }
    }

    // allow functions to be added, but also provide a way to test agreement on place
    pub fn add(&mut self, upvalue_count: usize) -> u32 {
        self.free.push(0);
        let count = self.offsets[self.offsets.len() - 1] + upvalue_count as usize;
        for _ in self.upvalues.len()..count {
            self.upvalues.push(Vec::new());
        }
        self.offsets.push(count);
        (self.free.len() - 1) as u32
    }

    pub fn limit(&self) -> usize {
        self.free.len()
    }

    pub fn get_function(&self, ch: ClosureHandle) -> FunctionHandle {
        Handle::from(ch.0 >> SHIFT)
    }

    pub fn get_upvalue(&self, ch: ClosureHandle, i: usize) -> UpvalueHandle {
        Handle::from(self.upvalues[self.offset(ch) + i][ch.index() & MASK])
    }

    fn offset(&self, ch: Handle<3>) -> usize {
        self.offsets[ch.index() >> SHIFT]
    }

    pub fn set_upvalue(&mut self, ch: ClosureHandle, i: usize, uh: UpvalueHandle) {
        let offset = self.offset(ch);
        self.upvalues[offset + i][ch.index() & MASK] = uh.0;
    }

    pub fn new_closure(&mut self, fh: FunctionHandle) -> ClosureHandle {
        let start = self.offsets[fh.index()];
        let stop = self.offsets[fh.index() + 1];
        if start == stop {
            return Handle::from(fh.0 << SHIFT);
        }
        let free = self.free[fh.index()] as usize;
        if free > MASK {
            panic!("Out of closure space")
        }
        let handle = Handle::from((fh.0 << SHIFT) + free as u32);
        if (free as usize) < self.upvalues[start].len() {
            self.free[fh.index()] = self.upvalues[start][free];
        } else {
            for i in start..stop {
                self.upvalues[i].push(0);
                self.free[fh.index()] += 1;
            }
        }
        handle
    }
}

impl Pool<CLOSURE> for Closures {
    fn byte_count(&self) -> usize {
        let mut capacity = 0;
        for vec in &self.upvalues {
            capacity += vec.capacity()
        }
        4 * capacity
    }
    fn trace(&self, handle: Handle<CLOSURE>, collector: &mut Collector) {
        let fh = self.get_function(handle);
        collector.push(fh);
        let start = self.offsets[fh.index()];
        let stop = self.offsets[fh.index() + 1];
        for i in start..stop {
            collector.push(UpvalueHandle::from(self.upvalues[i][handle.index() & MASK]));
        }
    }
    fn sweep(&mut self, marks: &BitArray) {
        for f in 0..self.offsets.len() - 1 {
            if self.offsets[f] == self.offsets[f + 1] {
                continue;
            }
            let vec = &mut self.upvalues[self.offsets[f]];
            self.free[f] = vec.len() as u32;
            for u in 0..vec.len() {
                let index = (f << SHIFT) + u;
                if !marks.has(index) {
                    vec[u] = self.free[f];
                    self.free[f] = u as u32;
                }
            }
        }
    }
    fn count(&self) -> usize {
        let mut count = 0;
        for i in 0..self.offsets.len() - 1 {
            if self.offsets[i + 1] > self.offsets[i] {
                count += self.upvalues[self.offsets[i]].len();
            }
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
        for i in vec![0, 0, 2, 0, 3, 1, 0, 1] {
            closures.add(i);
        }
        // try one
        let closure = closures.new_closure(Handle::from(2));
        assert_eq!(closures.get_function(closure).index(), 2);
        closures.set_upvalue(closure, 1, Handle::from(135));
        assert_eq!(closures.get_upvalue(closure, 1).index(), 135);

        // try another
        let closure2 = closures.new_closure(Handle::from(4));
        assert_eq!(closures.get_function(closure2).index(), 4);
        closures.set_upvalue(closure2, 2, Handle::from(135));
        assert_eq!(closures.get_upvalue(closure2, 2).index(), 135);

        // try an empty one
        let closure3 = closures.new_closure(Handle::from(6));
        assert_eq!(closures.get_function(closure3).index(), 6);
        assert_eq!(closures.count(), 2);
    }

    #[test]
    pub fn tracing() {
        let mut closures = Closures::new();
        for i in vec![0, 0, 2, 0, 3, 1, 0, 1] {
            closures.add(i);
        }
        let closure = closures.new_closure(Handle::from(2));
        closures.set_upvalue(closure, 1, Handle::from(135));

        let mut collector = Collector::new();
        closures.trace(closure, &mut collector);
        assert_eq!(collector.handles[FUNCTION], vec![2]);
        assert_eq!(collector.handles[UPVALUE], vec![0, 135]);
    }

    #[test]
    pub fn sweeping() {
        let mut closures = Closures::new();
        for i in vec![0, 0, 2, 0, 3, 1, 0, 1] {
            closures.add(i);
        }
        // try one
        let closure = closures.new_closure(Handle::from(2));
        closures.sweep(&BitArray::new());
        let closure2 = closures.new_closure(Handle::from(2));

        assert_eq!(closure, closure2);
    }
}
