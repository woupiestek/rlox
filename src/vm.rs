use std::collections::HashMap;

use crate::{
    compiler::compile,
    memory::{Heap, Obj},
    object::{Function, Upvalue, Value},
    stack::Stack,
};

const U8_COUNT: usize = 1 << 8;
const MAX_FRAMES: usize = 1 << 6;
const STACK_SIZE: usize = MAX_FRAMES * U8_COUNT;

struct CallFrame {
    ip: usize,
    slots: usize,
    // we seem to need a nullpointer, but isn't that a bit much?
    method: Obj<Function>,
}

pub struct VM {
    values: [Value; STACK_SIZE],
    // once we know what goes here,
    // we can arrays here too.
    frames: Stack<CallFrame>,
    open_upvalues: Stack<Obj<Upvalue>>,
    globals: HashMap<String, Value>,
    init_string: String,
    heap: Heap,
}

impl VM {
    pub fn new() -> Self {
        Self {
            values: [Value::Nil; STACK_SIZE],
            frames: Stack::new(MAX_FRAMES),
            open_upvalues: Stack::new(U8_COUNT),
            globals: HashMap::new(),
            init_string: "init".to_string(),
            heap: Heap::new(),
        }
    }
    pub fn interpret(&mut self, source: &str) -> Result<(), String> {
        compile(source, &mut self.heap);
        println!("{source}");
        Ok(())
    }
}
