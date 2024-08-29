use crate::{
    byte_code::ByteCode,
    heap::{Handle, Heap},
    object::{Closure, Value},
    strings::StringHandle,
};

// the top frame should be fast, cannot say it looks that way
pub struct CallStack<const MAX_SIZE: usize> {
    // current frame
    top: usize,
    // instruction pointers
    ips: [u32; MAX_SIZE],
    // offsets into operand stack
    slots: [u16; MAX_SIZE],
    // called functions
    closures: [Option<Handle>; MAX_SIZE],
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

    pub fn push(
        &mut self,
        slot: usize,
        closure: Handle,
        heap: &Heap,
        byte_code: &ByteCode,
    ) -> Result<(), String> {
        if self.top == 0 {
            return err!("Stack overflow.");
        }
        self.top -= 1;
        self.closures[self.top] = Some(closure);
        let fi = heap.get_ref::<Closure>(closure).function;
        self.ips[self.top] = byte_code.function_ref(fi).ip - 1;
        self.slots[self.top] = slot as u16;
        Ok(())
    }

    pub fn read_byte(&mut self, byte_code: &ByteCode) -> u8 {
        self.ips[self.top] += 1;
        byte_code.read_byte(self.ips[self.top] as usize)
    }

    pub fn read_constant(&mut self, byte_code: &ByteCode) -> Value {
        self.ips[self.top] += 1;
        byte_code.read_constant(
            self.ips[self.top] as usize,
        )
    }

    pub fn read_string(
        &mut self,
        byte_code: &ByteCode,
        heap: &Heap,
    ) -> Result<StringHandle, String> {
        let value = self.read_constant(byte_code);
        if let Value::String(handle) = value {
            Ok(handle)
        } else {
            err!("'{}' is not a string", value.to_string(heap, byte_code))
        }
    }

    pub fn upvalue(&self, index: usize, heap: &Heap) -> Result<Handle, String> {
        match self.closures[self.top] {
            Some(closure) => 
            {
            let closure = heap.get_ref::<Closure>(closure);
            if index >= closure.upvalues.len() {
              return err!("Upvalue index out of bound somehow {} out of {}", index,  closure.upvalues.len());
            }
            Ok(closure.upvalues[index])},
            None => err!("No closure in call frame"), // todo
        }
    }

    pub fn read_upvalue(&mut self, byte_code: &ByteCode, heap: &Heap) -> Result<Handle, String> {
        let index = byte_code.read_byte(self.ips[self.top] as usize) as usize;
        self.upvalue(index, heap)
    }

    pub fn slot(&self) -> usize {
        self.slots[self.top] as usize
    }

    pub fn jump_forward(&mut self, byte_code: &ByteCode) {
        self.ips[self.top] +=
            byte_code.read_short(self.ips[self.top] as usize + 1) as u32;
    }

    pub fn jump_back(&mut self, byte_code: &ByteCode) {
        self.ips[self.top] -=
            byte_code.read_short(self.ips[self.top] as usize + 1) as u32;
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

    pub fn trace(&self, collector: &mut Vec<Handle>) {
        for option in &self.closures {
            if let Some(closure) = option {
                collector.push(Handle::from(*closure))
            };
        }
    }

    pub fn print_stack_trace(&self, byte_code: &ByteCode, heap: &Heap) {
        for i in self.top..STACK_SIZE {
            if let Some(closure) = self.closures[i as usize] {
                eprintln!(
                    "  at {} line {}",
                    heap.to_string(closure, byte_code),
                    byte_code.get_line(self.ips[i as usize])
                )
            }
        }
    }

    #[cfg(feature = "trace")]
    pub fn print_trace(&self, byte_code: &ByteCode){
        let ip = self.ips[self.top];
        println!("ip: {}", ip);
        println!("line: {}", byte_code.get_line(ip));
    }
}
