use std::collections::{HashMap, HashSet};

use crate::{
    class::{Method, Path, Symbol},
    common::U8_COUNT,
    memory::{Heap, TypedHandle},
    object::{Upvalue, Value},
    stack::Stack,
};

const MAX_FRAMES: usize = 1 << 6;
const STACK_SIZE: usize = MAX_FRAMES * U8_COUNT;

#[derive(Copy, Clone)]
struct CallFrame {
    ip: usize,
    slots: usize,
    method: Path<Method>, // what is needed, and how do we do it?
}

pub struct VM {
    values: Stack<Value>,
    frames: Stack<CallFrame>,
    open_upvalues: Stack<TypedHandle<Upvalue>>,
    globals: HashMap<Symbol, Value>,
    symbol_pool: HashSet<Symbol>,
    init_symbol: Symbol,
    heap: Heap,
}

impl VM {
    pub fn new() -> Self {
        Self {
            values: Stack::new(STACK_SIZE),
            frames: Stack::new(MAX_FRAMES),
            open_upvalues: Stack::new(U8_COUNT),
            globals: HashMap::new(),
            symbol_pool: HashSet::new(),
            init_symbol: Symbol::copy("init"),
            heap: Heap::new(),
        }
    }
    pub fn interpret(&self, source: &str) -> Result<(), String> {
        println!("{source}");
        Ok(())
    }
}
