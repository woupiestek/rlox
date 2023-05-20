// run time data structures

use std::collections::HashMap;

use crate::memory::{Handle, Traceable, TypedHandle};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Value {
    Nil,
    True,
    False,
    Number(f64),
    Obj(Handle),
}

impl Traceable for String {
    const KIND: u8 = 0;
    fn trace(&self, _collector: &mut Vec<Handle>) {}
}

pub struct Method {
    pub name: TypedHandle<String>,
    pub arity: u16,
    code: Vec<u8>, // cannot just be opcodes.
    lines: Vec<u16>,
}

impl Method {
    pub fn write(&mut self, byte: u8, line: u16) {
        self.code.push(byte);
        self.lines.push(line);
    }
}

impl Traceable for Method {
    const KIND: u8 = 1;

    fn trace(&self, collector: &mut Vec<Handle>) {
        collector.push(self.name.downgrade());
    }
}

pub struct Class {
    pub name: TypedHandle<String>,
    up_value_count: u16,
    methods: Vec<Method>,
    constant: Vec<Value>,
}

impl Traceable for Class {
    const KIND: u8 = 2;

    fn trace(&self, collector: &mut Vec<Handle>) {
        collector.push(self.name.downgrade())
    }
}

pub struct Upvalue {
    location: usize, // don't know yet
    closed: Option<Value>,
}

impl Traceable for Upvalue {
    const KIND: u8 = 3;

    fn trace(&self, collector: &mut Vec<Handle>) {
        if let Some(Value::Obj(handle)) = self.closed {
            collector.push(handle);
        }
    }
}

// I guess the constructor can own the upvalues,
// though the class basically already determines how many are needed.
pub struct Constructor {
    class: *const Class,
    upvalues: Vec<TypedHandle<Upvalue>>,
}

impl Constructor {
    pub fn new(class: *const Class) -> Self {
        Self {
            class,
            upvalues: Vec::new(),
        }
    }
}

impl Traceable for Constructor {
    const KIND: u8 = 4;

    fn trace(&self, collector: &mut Vec<Handle>) {
        for upvalue in self.upvalues.iter() {
            collector.push(upvalue.downgrade());
        }
    }
}

pub struct Instance {
    constructor: TypedHandle<Constructor>,
    fields: HashMap<String, Value>,
}

impl Traceable for Instance {
    const KIND: u8 = 5;

    fn trace(&self, collector: &mut Vec<Handle>) {
        for value in self.fields.values() {
            if let Value::Obj(handle) = value {
                collector.push(*handle)
            }
        }
    }
}

impl Instance {
    pub fn new(constructor: TypedHandle<Constructor>) -> Self {
        Self {
            constructor,
            fields: HashMap::new(),
        }
    }
}

pub struct BoundMethod {
    receiver: TypedHandle<Instance>,
    method: TypedHandle<Method>,
}

impl BoundMethod {
    pub fn new(receiver: TypedHandle<Instance>, method: TypedHandle<Method>) -> Self {
        Self { receiver, method }
    }
}

impl Traceable for BoundMethod {
    const KIND: u8 = 6;

    fn trace(&self, collector: &mut Vec<Handle>) {
        todo!()
    }
}

#[derive(Copy, Clone)]
pub struct NativeFn(fn(args: &[Value]) -> Value);
impl std::fmt::Debug for NativeFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<native function>")
    }
}

impl Traceable for NativeFn {
    const KIND: u8 = 7;

    fn trace(&self, _collector: &mut Vec<Handle>) {}
}
