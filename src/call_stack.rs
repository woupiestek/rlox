use std::cmp;

use crate::{
    byte_code::ByteCode,
    heap::{Handle, Heap},
    object::{Closure, Value},
    strings::StringHandle,
};

// the top frame should be fast, cannot say it looks that way
pub struct CallStack<const MAX_SIZE: usize> {
    // current frame
    top: isize,
    // instruction pointers
    ips: [i32; MAX_SIZE],
    // offsets into operand stack
    slots: [u16; MAX_SIZE],
    // called functions
    closures: [Option<Handle>; MAX_SIZE],
}

impl<const STACK_SIZE: usize> CallStack<STACK_SIZE> {
    pub fn new() -> Self {
        Self {
            top: -1,
            ips: [-1; STACK_SIZE], // we could just not use the first index in bytecode, or compensate some other way...
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
        self.top += 1;
        if self.top as usize == STACK_SIZE {
            return err!("Stack overflow.");
        }
        self.closures[self.top as usize] = Some(closure);
        let fi = heap.get_ref::<Closure>(closure).function;
        self.ips[self.top as usize] = byte_code.function_ref(fi).ip as i32 - 1;
        self.slots[self.top as usize] = slot as u16;
        Ok(())
    }

    pub fn read_byte(&mut self, byte_code: &ByteCode) -> u8 {
        self.ips[self.top as usize] += 1;
        byte_code.read_byte(self.ips[self.top as usize] as usize)
    }

    pub fn read_constant(&mut self, byte_code: &ByteCode) -> Value {
        self.ips[self.top as usize] += 1;
        byte_code.read_constant(
            self.ips[self.top as usize] as usize,
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
        match self.closures[self.top as usize] {
            Some(closure) => Ok(heap.get_ref::<Closure>(closure).upvalues[index]),
            None => err!("No closure in call frame"), // todo
        }
    }

    pub fn read_upvalue(&mut self, byte_code: &ByteCode, heap: &Heap) -> Result<Handle, String> {
        self.ips[self.top as usize] += 1;
        match self.closures[self.top as usize] {
            Some(closure) => {
                let closure = heap.get_ref::<Closure>(closure);
                Ok(closure.upvalues
                    [byte_code.read_byte(self.ips[self.top as usize] as usize) as usize])
            }
            None => err!("No closure in call frame"), // todo
        }
    }

    pub fn slot(&self) -> usize {
        self.slots[self.top as usize] as usize
    }

    pub fn jump_forward(&mut self, byte_code: &ByteCode) {
        self.ips[self.top as usize] +=
            byte_code.read_short(self.ips[self.top as usize] as usize + 1) as i32;
    }

    pub fn jump_back(&mut self, byte_code: &ByteCode) {
        self.ips[self.top as usize] -=
            byte_code.read_short(self.ips[self.top as usize] as usize + 1) as i32;
    }

    pub fn skip(&mut self) {
        self.ips[self.top as usize] += 2
    }

    pub fn pop(&mut self) {
        self.top -= 1;
    }

    pub fn is_empty(&self) -> bool {
        self.top < 0
    }

    pub fn trace(&self, collector: &mut Vec<Handle>) {
        for option in &self.closures {
            if let Some(closure) = option {
                collector.push(Handle::from(*closure))
            };
        }
    }

    pub fn print_stack_trace(&self, byte_code: &ByteCode, heap: &Heap) {
        for i in (0..=cmp::min(STACK_SIZE - 1,self.top as usize)).rev() {
            if let Some(closure) = self.closures[i as usize] {
                eprintln!(
                    "  at {} line {}",
                    heap.to_string(closure, byte_code),
                    byte_code.get_line(self.ips[i as usize] as u32)
                )
            }
        }
    }

    #[cfg(feature = "trace")]
    pub fn print_trace(&self, byte_code: &ByteCode){
        let ip = self.ips[self.top as usize];
        println!("ip: {}", ip);
        println!("line: {}", byte_code.get_line(ip as u32));
    }
}
