use std::{collections::HashMap, time};

use crate::{
    common::U8_COUNT,
    compiler::compile,
    memory::{Heap, Obj},
    object::{Closure, Native, Upvalue, Value},
    stack::Stack,
};

const MAX_FRAMES: usize = 1 << 6;
const STACK_SIZE: usize = MAX_FRAMES * U8_COUNT;

fn clock_native(_args: &[Value]) -> Value {
    match time::SystemTime::now().duration_since(time::UNIX_EPOCH) {
        Ok(duration) => Value::Number(duration.as_millis() as f64),
        Err(_) => Value::Nil, // just like how js would solve it
    }
}

const CLOCK_NATIVE: Native = Native(clock_native);

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
    frames: Stack<CallFrame<'vm>>,
    open_upvalues: Stack<Obj<Upvalue>>,
    globals: HashMap<String, Value>,
    init_string: String,
    heap: Heap,
}

impl<'vm> VM<'vm> {
    pub fn new() -> Self {
        let mut s = Self {
            values: [Value::Nil; STACK_SIZE],
            count: 0,
            frames: Stack::new(MAX_FRAMES),
            open_upvalues: Stack::new(U8_COUNT),
            globals: HashMap::new(),
            init_string: "init".to_string(),
            heap: Heap::new(),
        };
        s.define_native("clock", CLOCK_NATIVE);
        s
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
        self.values[self.values.len() - 1 - distance]
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
            slots: 10, // self.count,
            closure: Some(closure),
        });
        Ok(())
    }

    // hiero

    fn define_native(&mut self, name: &str, native_fn: Native) {
        let value = Value::Object(self.heap.store(native_fn).downgrade());
        self.globals.insert(name.to_string(), value);
    }

    pub fn interpret(&mut self, source: &str) -> Result<(), String> {
        println!("{}", source);
        compile(source, &mut self.heap)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_error_on_init() {
        // access violation
        VM::new();
    }

    // #[test]
    fn interpret_empty_string() {
        let mut vm = VM::new();
        assert!(vm.interpret("").is_ok())
    }
}
