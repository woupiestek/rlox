use crate::{
    closures::ClosureHandle, functions::ChunkFrame, heap::Heap, strings::StringHandle,
    upvalues::UpvalueHandle, values::Value,
};

// get these on the stack.
pub struct CallFrame {
    ip: isize,
    lp: usize,
    cp: usize,
    pub slot: usize,
    pub closure: ClosureHandle,
}

impl CallFrame {
    pub fn placeholder() -> Self {
        Self {
            ip: -1,
            lp: 0,
            cp: 0,
            slot: 0,
            closure: ClosureHandle::from(0),
        }
    }
    pub fn new(slot: usize, closure: ClosureHandle, heap: &Heap) -> Self {
        let function = heap.closures.get_function(closure);
        let &ChunkFrame { ip, lp, cp } = heap.functions.get_frame(function);
        Self {
            // stick to putting the pointer next to the code to read
            ip: ip as isize - 1,
            lp,
            cp,
            slot,
            closure,
        }
    }

    fn pop(&mut self) -> usize {
        self.ip += 1;
        self.ip as usize
    }

    pub fn read_byte(&mut self, heap: &Heap) -> u8 {
        heap.functions.chunk.read_byte(self.pop())
    }

    pub fn read_constant(&mut self, heap: &Heap) -> Value {
        heap.functions
            .chunk
            .read_constant(self.cp + self.read_byte(heap) as usize)
    }

    pub fn read_string(&mut self, heap: &Heap) -> Result<StringHandle, String> {
        StringHandle::try_from(self.read_constant(heap))
    }

    pub fn get_upvalue(&self, heap: &Heap, index: usize) -> UpvalueHandle {
        heap.closures.upvalues[heap.closures.up(self.closure) + index]
    }

    pub fn read_upvalue<'b>(&mut self, heap: &Heap) -> UpvalueHandle {
        let index = self.read_byte(heap) as usize;
        self.get_upvalue(heap, index)
    }

    pub fn jump_forward(&mut self, heap: &Heap) {
        self.ip += heap.functions.chunk.read_short(self.ip as usize + 1) as isize;
    }

    pub fn jump_back(&mut self, heap: &Heap) {
        self.ip -= heap.functions.chunk.read_short(self.ip as usize + 1) as isize;
    }

    pub fn skip(&mut self) {
        self.ip += 2
    }

    pub fn print(&self, heap: &Heap) {
        let fh = heap.closures.get_function(self.closure);
        eprintln!(
            "  at {} line {}",
            heap.functions.to_string(fh, heap),
            heap.functions.chunk.get_line(self.lp, self.ip as usize)
        )
    }
}
