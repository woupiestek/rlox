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
const COUNTS: usize = 256;

pub struct Closures {
    free: [u32; COUNTS],
    offsets: [u16; COUNTS],
    values: Vec<Vec<u32>>,
}

impl Closures {
    pub fn new() -> Self {
        let mut offsets = [0; COUNTS];
        offsets[0] = 1;
        offsets[1] = 2;
        Self {
            free: [0; COUNTS],
            offsets,
            values: vec![Vec::new(); 3],
        }
    }

    fn upvalue_count(ch: ClosureHandle) -> usize {
        ch.index() >> SHIFT
    }

    pub fn get_function(&self, ch: ClosureHandle) -> FunctionHandle {
        Handle::from(
            self.values[self.offsets[Closures::upvalue_count(ch)] as usize - 1][ch.index() & MASK],
        )
    }

    pub fn get_upvalue(&self, ch: ClosureHandle, i: usize) -> UpvalueHandle {
        Handle::from(self.values[self.offset(ch) + i][ch.index() & MASK])
    }

    fn offset(&self, ch: Handle<3>) -> usize {
        self.offsets[Closures::upvalue_count(ch)] as usize
    }

    pub fn set_upvalue(&mut self, ch: ClosureHandle, i: usize, uh: UpvalueHandle) {
        self.values[self.offsets[Closures::upvalue_count(ch)] as usize + i][ch.index() & MASK] =
            uh.0;
    }

    fn force_offset(&mut self, uc: usize) -> usize {
        if self.offsets[uc] == 0 {
            self.values.push(Vec::new());
            self.offsets[uc] = self.values.len() as u16;
            for _ in 0..uc {
                self.values.push(Vec::new());
            }
        }
        self.offsets[uc] as usize
    }

    pub fn new_closure(&mut self, fh: FunctionHandle, uc: usize) -> ClosureHandle {
        let free = self.free[uc] as usize;
        if free > MASK {
            panic!("Out of closure space")
        }
        let offset = self.force_offset(uc);
        let values = &mut self.values[offset - 1];
        if free < values.len() {
            // reuse memory
            self.free[uc] = values[free];
            self.values[offset - 1][free] = fh.0;
        } else {
            self.free[uc] += 1;
            values.push(fh.0);
            for i in 0..uc {
                self.values[offset + i].push(0);
            }
        }
        ClosureHandle::from((uc << SHIFT) as u32 + free as u32)
    }
}

impl Pool<CLOSURE> for Closures {
    fn byte_count(&self) -> usize {
        let mut capacity = 0;
        for vec in &self.values {
            capacity += vec.capacity()
        }
        4 * capacity
    }
    fn trace(&self, handle: Handle<CLOSURE>, collector: &mut Collector) {
        let uc = Closures::upvalue_count(handle);
        let offset = self.offsets[uc] as usize;
        let index = handle.index() & MASK;
        // let fh = self.get_function(handle);
        collector.push(FunctionHandle::from(self.values[offset - 1][index]));
        for i in 0..uc {
            collector.push(UpvalueHandle::from(self.values[offset + i][index]));
        }
    }
    fn sweep(&mut self, marks: &BitArray) {
        for uc in 0..COUNTS {
            let offset = self.offsets[uc] as usize;
            if offset == 0 {
                continue;
            }
            let values = &mut self.values[offset - 1];
            self.free[uc] = values.len() as u32;
            for u in 0..values.len() {
                if !marks.has((uc << SHIFT) + u) {
                    values[u] = self.free[uc];
                    self.free[uc] = u as u32;
                }
            }
        }
    }
    fn count(&self) -> usize {
        let mut count = 0;
        for i in 0..COUNTS {
            if self.offsets[i] > 0 {
                count += self.values[self.offsets[i] as usize - 1].len();
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
        assert_eq!(closures.count(), 3);
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
