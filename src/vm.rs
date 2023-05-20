use std::collections::HashMap;

use crate::{
    memory::{Heap, TypedHandle},
    object::{Method, Upvalue, Value},
    stack::Stack,
};

const U8_COUNT: usize = 1 << 8;
const MAX_FRAMES: usize = 1 << 6;
const STACK_SIZE: usize = MAX_FRAMES * U8_COUNT;

struct CallFrame {
    ip: usize,
    slots: usize,
    method: TypedHandle<Method>,
}

pub struct VM {
    values: Stack<Value>,
    frames: Stack<CallFrame>,
    open_upvalues: Stack<TypedHandle<Upvalue>>,
    globals: HashMap<String, Value>,
    init_string: String,
    heap: Heap,
}

impl VM {
    pub fn new() -> Self {
        Self {
            values: Stack::new(STACK_SIZE),
            frames: Stack::new(MAX_FRAMES),
            open_upvalues: Stack::new(U8_COUNT),
            globals: HashMap::new(),
            init_string: "init".to_string(),
            heap: Heap::new(),
        }
    }
    pub fn interpret(&self, source: &str) -> Result<(), String> {
        println!("{source}");
        Ok(())
    }
}
