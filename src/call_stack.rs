use crate::{
    functions::{Chunk, Functions},
    heap::{Collector, Heap, ObjectHandle},
    object::{Closure, Value},
    strings::StringHandle, upvalues::UpvalueHandle,
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
    closures: [Option<ObjectHandle>; MAX_SIZE],
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

    pub fn push(&mut self, slot: usize, closure: ObjectHandle) -> Result<(), String> {
        if self.top == 0 {
            return err!("Stack overflow.");
        }
        self.top -= 1;
        self.closures[self.top] = Some(closure);
        self.ips[self.top] = -1;
        self.slots[self.top] = slot as u16;
        Ok(())
    }

    fn get_chunk<'b>(&self, functions: &'b Functions, heap: &Heap) -> &'b Chunk {
        let fi = heap
            .get_ref::<Closure>(self.closures[self.top].unwrap())
            .function;
        functions.chunk_ref(fi)
    }

    pub fn read_byte(&mut self, functions: &Functions, heap: &Heap) -> u8 {
        self.ips[self.top] += 1;
        self.get_chunk(functions, heap)
            .read_byte(self.ips[self.top] as usize)
    }

    pub fn read_constant(&mut self, functions: &Functions, heap: &Heap) -> Value {
        self.ips[self.top] += 1;
        self.get_chunk(functions, heap)
            .read_constant(self.ips[self.top] as usize)
    }

    pub fn read_string(
        &mut self,
        functions: &Functions,
        heap: &Heap,
    ) -> Result<StringHandle, String> {
        let value = self.read_constant(functions, heap);
        if let Value::String(handle) = value {
            Ok(handle)
        } else {
            err!("'{}' is not a string", value.to_string(heap, functions))
        }
    }

    pub fn upvalue(&self, index: usize, heap: &Heap) -> Result<UpvalueHandle, String> {
        match self.closures[self.top] {
            Some(closure) => {
                let closure = heap.get_ref::<Closure>(closure);
                Ok(closure.upvalues[index])
            }
            None => err!("No closure in call frame"), // todo
        }
    }

    pub fn read_upvalue(&mut self, functions: &Functions, heap: &Heap) -> Result<UpvalueHandle, String> {
        let index = self.read_byte(functions, heap) as usize;
        self.upvalue(index, heap)
    }

    pub fn slot(&self) -> usize {
        self.slots[self.top] as usize
    }

    pub fn jump_forward(&mut self, functions: &Functions, heap: &Heap) {
        self.ips[self.top] += self
            .get_chunk(functions, heap)
            .read_short(self.ips[self.top] as usize + 1) as i32;
    }

    pub fn jump_back(&mut self, functions: &Functions, heap: &Heap) {
        self.ips[self.top] -= self
            .get_chunk(functions, heap)
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
        for option in &self.closures {
            if let Some(closure) = option {
                collector.objects.push(ObjectHandle::from(*closure))
            };
        }
    }

    pub fn print_stack_trace(&self, functions: &Functions, heap: &Heap) {
        for i in self.top..STACK_SIZE {
            if let Some(closure) = self.closures[i as usize] {
                eprintln!(
                    "  at {} line {}",
                    heap.to_string(closure, functions),
                    self.get_chunk(functions, heap)
                        .get_line(self.ips[i as usize])
                )
            }
        }
    }

    #[cfg(feature = "trace")]
    pub fn print_trace(&self, functions: &Functions, heap: &Heap) {
        let ip = self.ips[self.top];
        println!("ip: {}", ip);
        let fh = heap
            .get_ref::<Closure>(self.closures[self.top].unwrap())
            .function;
        println!("{}:", functions.to_string(fh, heap));
        let chunk = functions.chunk_ref(fh);
        println!("line: {}", chunk.get_line(ip));
    }
}
