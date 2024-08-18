use crate::{
    chunk::Chunk,
    loxtr::Loxtr,
    memory::{Traceable, GC},
    object::{Closure, Upvalue, Value},
};

pub const MAX_FRAMES: usize = 64;

// the top frame should be fast, cannot say it looks that way
pub struct CallStack {
    // current frame
    pub top: isize,
    // instruction pointers
    pub ips: [isize; MAX_FRAMES],
    // offsets into operand stack
    slots: [usize; MAX_FRAMES],
    // called functions
    pub closures: [Option<GC<Closure>>; MAX_FRAMES],
}

impl CallStack {
    pub fn new() -> Self {
        Self {
            top: -1,
            ips: [-1; MAX_FRAMES],
            slots: [0; MAX_FRAMES],
            closures: [Option::None; MAX_FRAMES],
        }
    }

    pub fn push(&mut self, slot: usize, closure: GC<Closure>) -> Result<(), String> {
        self.top += 1;
        if self.top as usize == MAX_FRAMES {
            return err!("Stack overflow.");
        }
        self.ips[self.top as usize] = -1;
        self.closures[self.top as usize] = Some(closure);
        self.slots[self.top as usize] = slot;
        Ok(())
    }

    fn chunk(&self) -> Option<&Chunk> {
        match &self.closures[self.top as usize] {
            Some(closure) => Some(&closure.function.chunk),
            None => None, // todo
        }
    }

    pub fn read_byte(&mut self) -> u8 {
        self.ips[self.top as usize] += 1;
        match self.chunk() {
            Some(chunk) => chunk.read_byte(self.ips[self.top as usize] as usize),
            None => 0, // todo
        }
    }

    pub fn read_constant(&mut self) -> Value {
        self.ips[self.top as usize] += 1;
        match self.chunk() {
            Some(chunk) => chunk.read_constant(self.ips[self.top as usize] as usize),
            None => Value::Nil, // todo
        }
    }

    pub fn read_string(&mut self) -> Result<GC<Loxtr>, String> {
        let value = self.read_constant();
        Loxtr::nullable(value).ok_or_else(|| format!("'{}' is not a string", value))
    }

    pub fn upvalue(&self, index: usize) -> Result<GC<Upvalue>, String> {
        match self.closures[self.top as usize] {
            Some(closure) => Ok(closure.upvalues[index]),
            None => err!("No closure in call frame"), // todo
        }
    }

    pub fn read_upvalue(&mut self) -> Result<GC<Upvalue>, String> {
        self.ips[self.top as usize] += 1;
        match self.closures[self.top as usize] {
            Some(closure) => Ok(closure.upvalues[closure
                .function
                .chunk
                .read_byte(self.ips[self.top as usize] as usize)
                as usize]),
            None => err!("No closure in call frame"), // todo
        }
    }

    pub fn slot(&self) -> usize {
        self.slots[self.top as usize]
    }

    pub fn jump_forward(&mut self) {
        if let Some(chunk) = self.chunk() {
            self.ips[self.top as usize] +=
                chunk.read_short(self.ips[self.top as usize] as usize + 1) as isize;
        }
        // todo: improve data structure
    }

    pub fn jump_back(&mut self) {
        if let Some(chunk) = self.chunk() {
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
}
