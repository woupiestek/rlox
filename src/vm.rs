use std::{
    collections::HashMap,
    time::{self, Instant, UNIX_EPOCH},
};

use crate::{
    compiler::compile,
    memory::{Heap, Obj},
    object::{Closure, Function, NativeFn, Upvalue, Value},
    stack::Stack,
};

const U8_COUNT: usize = 1 << 8;
const MAX_FRAMES: usize = 1 << 6;
const STACK_SIZE: usize = MAX_FRAMES * U8_COUNT;

fn clock_native(_args: &[Value]) -> Value {
    match time::SystemTime::now().duration_since(time::UNIX_EPOCH) {
        Ok(duration) => Value::Number(duration.as_millis() as f64),
        Err(_) => Value::Nil, // just like how js would solve it
    }
}

const CLOCK_NATIVE: NativeFn = NativeFn(clock_native);

#[derive(Copy, Clone)]
struct CallFrame<'vm> {
    ip: usize,
    slots: usize,
    // we seem to need a nullpointer, but isn't that a bit much?
    closure: Option<&'vm Closure>,
}

pub struct VM<'vm> {
    values: [Value; STACK_SIZE],
    count: usize,
    // once we know what goes here,
    // we can arrays here too.
    frames: Stack<CallFrame<'vm>>,
    open_upvalues: Stack<Obj<Upvalue>>,
    globals: HashMap<String, Value>,
    init_string: String,
    heap: Heap,
}

impl<'vm> VM<'vm> {
    pub fn new() -> Self {
        Self {
            values: [Value::Nil; STACK_SIZE],
            count: 0,
            frames: Stack::new(MAX_FRAMES),
            open_upvalues: Stack::new(U8_COUNT),
            globals: HashMap::new(),
            init_string: "init".to_string(),
            heap: Heap::new(),
        }
    }

    fn push(&mut self, value: Value) {
        self.values[self.count] = value;
        self.count += 1;
    }

    fn pop(&mut self) -> Value {
        self.count -= 1;
        self.values[self.count]
    }

    fn peek(&self, distance: usize) -> Value {
        self.values[self.count - 1 - distance]
    }

    fn call(&mut self, closure: &'vm Closure, arg_count: u8) -> Result<(), String> {
        if arg_count != closure.function.arity {
            return Err(format!(
                "Expected {} arguments but got {}.",
                closure.function.arity, arg_count
            ));
        }

        if self.frames.len() == MAX_FRAMES {
            return Err("Stack overflow.".to_string());
        }
        self.frames.push(CallFrame {
            ip: 0,
            slots: self.count,
            closure: Some(closure),
        });
        Ok(())
    }

    // hiero

    fn define_native(&mut self, name: &str, native_fn: NativeFn) {
        let value = Value::Object(self.heap.store(native_fn).downgrade());
        self.globals.insert(name.to_string(), value);
    }

    pub fn interpret(&mut self, source: &str) -> Result<(), String> {
        compile(source, &mut self.heap);
        println!("{source}");
        Ok(())
    }
}
