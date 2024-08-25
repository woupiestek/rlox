use crate::{
    chunk::Chunk,
    heap::{Handle, Heap},
    object::{Closure, Function, Value},
    strings::StringHandle,
};

// the top frame should be fast, cannot say it looks that way
pub struct CallStack<const MAX_SIZE: usize> {
    // current frame
    top: i16,
    // instruction pointers
    ips: [isize; MAX_SIZE],
    // offsets into operand stack
    slots: [u16; MAX_SIZE],
    // called functions
    closures: [Option<Handle>; MAX_SIZE],
}

impl<const MAX_SIZE: usize> CallStack<MAX_SIZE> {
    pub fn new() -> Self {
        Self {
            top: -1,
            ips: [-1; MAX_SIZE],
            slots: [0; MAX_SIZE],
            closures: [Option::None; MAX_SIZE],
        }
    }

    pub fn push(&mut self, slot: usize, closure: Handle) -> Result<(), String> {
        self.top += 1;
        if self.top as usize == MAX_SIZE {
            return err!("Stack overflow.");
        }
        self.ips[self.top as usize] = -1;
        self.closures[self.top as usize] = Some(closure);
        self.slots[self.top as usize] = slot as u16;
        Ok(())
    }

    fn chunk<'hp>(&self, heap: &'hp Heap) -> Option<&'hp Chunk> {
        match self.closures[self.top as usize] {
            Some(closure) => {
                let closure = heap.get_ref::<Closure>(closure);
                let function = heap.get_ref::<Function>(closure.function);
                Some(&function.chunk)
            }
            None => None, // todo
        }
    }

    pub fn read_byte(&mut self, heap: &Heap) -> u8 {
        self.ips[self.top as usize] += 1;
        match self.chunk(heap) {
            Some(chunk) => chunk.read_byte(self.ips[self.top as usize] as usize),
            None => 0, // todo
        }
    }

    pub fn read_constant(&mut self, heap: &Heap) -> Value {
        self.ips[self.top as usize] += 1;
        match self.chunk(heap) {
            Some(chunk) => chunk.read_constant(self.ips[self.top as usize] as usize),
            None => Value::Nil, // todo
        }
    }

    pub fn read_string(&mut self, heap: &Heap) -> Result<StringHandle, String> {
        let value = self.read_constant(heap);
        if let Value::String(handle) = value {
            Ok(handle)
        } else {
            err!("'{}' is not a string", value.to_string(heap))
        }
    }

    pub fn upvalue(&self, index: usize, heap: &Heap) -> Result<Handle, String> {
        match self.closures[self.top as usize] {
            Some(closure) => Ok(heap.get_ref::<Closure>(closure).upvalues[index]),
            None => err!("No closure in call frame"), // todo
        }
    }

    pub fn read_upvalue(&mut self, heap: &Heap) -> Result<Handle, String> {
        self.ips[self.top as usize] += 1;
        match self.closures[self.top as usize] {
            Some(closure) => {
                let closure = heap.get_ref::<Closure>(closure);
                let function = heap.get_ref::<Function>(closure.function);
                Ok(closure.upvalues[function
                    .chunk
                    .read_byte(self.ips[self.top as usize] as usize)
                    as usize])
            }
            None => err!("No closure in call frame"), // todo
        }
    }

    pub fn slot(&self) -> usize {
        self.slots[self.top as usize] as usize
    }

    pub fn jump_forward(&mut self, heap: &Heap) {
        if let Some(chunk) = self.chunk(heap) {
            self.ips[self.top as usize] +=
                chunk.read_short(self.ips[self.top as usize] as usize + 1) as isize;
        }
        // todo: improve data structure
    }

    pub fn jump_back(&mut self, heap: &Heap) {
        if let Some(chunk) = self.chunk(heap) {
            self.ips[self.top as usize] -=
                chunk.read_short(self.ips[self.top as usize] as usize + 1) as isize;
        }
        // todo: improve data structure
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

    pub fn trace(&self, collector:  &mut Vec<Handle>) {
        for option in &self.closures {
            if let Some(closure) = option {
                collector.push(Handle::from(*closure))
            };
        }
    }

    pub fn print_stack_trace(&self, heap: &Heap) {
        for i in 0..self.top {
            if let Some(closure) = self.closures[i as usize] {
                let closure = heap.get_ref::<Closure>(closure);
                let function: &Function = heap.get_ref::<Function>(closure.function);
                eprintln!(
                    "  at {} line {}",
                    heap.to_string(closure.function),
                    function.chunk.lines[self.ips[i as usize] as usize]
                )
            }
        }
    }
}
