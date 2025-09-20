use crate::{
    closures::ClosureHandle,
    functions::Chunk,
    heap::{Collector, Heap},
    strings::StringHandle,
    upvalues::UpvalueHandle,
    values::Value,
};

// get these on the stack.
pub struct CallFrame {
    ip: isize,
    pub slot: usize,
    closure: ClosureHandle,
}

impl CallFrame {
    pub fn new(slot: usize, closure: ClosureHandle) -> Self {
        Self {
            // move, then use pointer. Hence the weird start
            ip: -1,
            slot,
            closure,
        }
    }

    fn pop(&mut self) -> usize {
        self.ip += 1;
        self.ip as usize
    }

    fn get_chunk<'b>(&self, heap: &'b Heap) -> &'b Chunk {
        let fi = heap.closures.get_function(self.closure);
        heap.functions.chunk_ref(fi)
    }

    pub fn read_byte(&mut self, heap: &Heap) -> u8 {
        self.get_chunk(heap).read_byte(self.pop())
    }

    pub fn read_constant(&mut self, heap: &Heap) -> Value {
        self.get_chunk(heap).read_constant(self.pop())
    }

    pub fn read_string(&mut self, heap: &Heap) -> Result<StringHandle, String> {
        let value = self.read_constant(heap);
        StringHandle::try_from(value)
    }

    pub fn upvalue(&self, index: usize, heap: &Heap) -> UpvalueHandle {
        heap.closures.get_upvalue(self.closure, index)
    }

    pub fn read_upvalue(&mut self, heap: &Heap) -> UpvalueHandle {
        let index = self.read_byte(heap) as usize;
        self.upvalue(index, heap)
    }

    pub fn jump_forward(&mut self, heap: &Heap) {
        self.ip += self.get_chunk(heap).read_short(self.ip as usize + 1) as isize;
    }

    pub fn jump_back(&mut self, heap: &Heap) {
        self.ip -= self.get_chunk(heap).read_short(self.ip as usize + 1) as isize;
    }

    pub fn skip(&mut self) {
        self.ip += 2
    }

    pub fn trace(&self, collector: &mut Collector) {
        collector.push(self.closure)
    }

    pub fn print(&self, heap: &Heap) {
        let fh = heap.closures.get_function(self.closure);
        eprintln!(
            "  at {} line {}",
            heap.functions.to_string(fh, heap),
            self.get_chunk(heap).get_line(self.ip as i32)
        )
    }
}
