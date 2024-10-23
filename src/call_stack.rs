use crate::{
    closures2::ClosureHandle,
    functions::Chunk,
    heap::{Collector, Heap},
    strings::StringHandle,
    upvalues::UpvalueHandle,
    values::Value,
};

// the top frame should be fast, cannot say it looks that way
pub struct CallStack<const MAX_SIZE: usize> {
    // current frame
    top: usize,
    // instruction pointers
    ips: [i32; MAX_SIZE],
    // offsets into operand stack
    slots: [u16; MAX_SIZE],
    // called functions
    closures: [Option<ClosureHandle>; MAX_SIZE],
}

impl<const STACK_SIZE: usize> CallStack<STACK_SIZE> {
    pub fn new() -> Self {
        Self {
            top: STACK_SIZE,
            ips: [0; STACK_SIZE],
            slots: [0; STACK_SIZE],
            closures: [Option::None; STACK_SIZE],
        }
    }

    pub fn push(&mut self, slot: usize, closure: ClosureHandle) -> Result<(), String> {
        if self.top == 0 {
            return err!("Stack overflow.");
        }
        self.top -= 1;
        self.closures[self.top] = Some(closure);
        self.ips[self.top] = -1;
        self.slots[self.top] = slot as u16;
        Ok(())
    }

    fn get_chunk<'b>(&self, heap: &'b Heap) -> &'b Chunk {
        let fi = heap.closures.get_function(self.closures[self.top].unwrap());
        heap.functions.chunk_ref(fi)
    }

    pub fn read_byte(&mut self, heap: &Heap) -> u8 {
        self.ips[self.top] += 1;
        self.get_chunk(heap).read_byte(self.ips[self.top] as usize)
    }

    pub fn read_constant(&mut self, heap: &Heap) -> Value {
        self.ips[self.top] += 1;
        self.get_chunk(heap)
            .read_constant(self.ips[self.top] as usize)
    }

    pub fn read_string(&mut self, heap: &Heap) -> Result<StringHandle, String> {
        let value = self.read_constant(heap);
        StringHandle::try_from(value)
    }

    pub fn upvalue(&self, index: usize, heap: &Heap) -> Result<UpvalueHandle, String> {
        match self.closures[self.top] {
            Some(closure) => Ok(heap.closures.get_upvalue(closure, index)),
            None => err!("No closure in call frame"), // todo
        }
    }

    pub fn read_upvalue(&mut self, heap: &Heap) -> Result<UpvalueHandle, String> {
        let index = self.read_byte(heap) as usize;
        self.upvalue(index, heap)
    }

    pub fn slot(&self) -> usize {
        self.slots[self.top] as usize
    }

    pub fn jump_forward(&mut self, heap: &Heap) {
        self.ips[self.top] += self
            .get_chunk(heap)
            .read_short(self.ips[self.top] as usize + 1) as i32;
    }

    pub fn jump_back(&mut self, heap: &Heap) {
        self.ips[self.top] -= self
            .get_chunk(heap)
            .read_short(self.ips[self.top] as usize + 1) as i32;
    }

    pub fn skip(&mut self) {
        self.ips[self.top] += 2
    }

    pub fn pop(&mut self) {
        self.top += 1;
    }

    pub fn is_empty(&self) -> bool {
        self.top >= STACK_SIZE
    }

    pub fn trace(&self, collector: &mut Collector) {
        for &option in &self.closures {
            if let Some(closure) = option {
                collector.push(closure)
            };
        }
    }

    pub fn print_stack_trace(&self, heap: &Heap) {
        for i in self.top..STACK_SIZE {
            if let Some(closure) = self.closures[i as usize] {
                let fh = heap.closures.get_function(closure);
                eprintln!(
                    "  at {} line {}",
                    heap.functions.to_string(fh, heap),
                    self.get_chunk(heap).get_line(self.ips[i as usize])
                )
            }
        }
    }

    #[cfg(feature = "trace")]
    pub fn print_trace(&self, heap: &Heap) {
        let ip = self.ips[self.top];
        println!("ip: {}", ip);
        let fh = heap
            .closures
            .function_handle(self.closures[self.top].unwrap());
        println!("{}:", heap.functions.to_string(fh, heap));
        let chunk = heap.functions.chunk_ref(fh);
        println!("line: {}", chunk.get_line(ip));
    }
}
