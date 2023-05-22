// run time data structures

use std::collections::HashMap;

use crate::{
    chunk::Chunk,
    memory::{Handle, Obj, Traceable},
};

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

pub struct Function {
    pub name: Option<Obj<String>>,
    pub arity: u8,
    pub upvalue_count: u8,
    pub chunk: Chunk,
}

impl Function {
    pub fn new() -> Self {
        Self {
            name: None,
            arity: 0,
            upvalue_count: 0,
            chunk: Chunk::new(),
        }
    }
}

impl Traceable for Function {
    const KIND: u8 = 1;

    fn trace(&self, _collector: &mut Vec<Handle>) {}
}

pub struct Class {
    pub name: Obj<String>,
    pub methods: HashMap<String, Obj<Closure>>,
}

impl Class {
    pub fn new(name: Obj<String>) -> Self {
        Self {
            name,
            methods: HashMap::new(),
        }
    }
}

impl Traceable for Class {
    const KIND: u8 = 2;
    fn trace(&self, collector: &mut Vec<Handle>) {
        collector.push(self.name.downgrade());
        for method in self.methods.values() {
            collector.push(method.downgrade());
        }
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
pub struct Closure {
    class: Obj<Function>,
    upvalues: Vec<Obj<Upvalue>>,
}

impl Closure {
    pub fn new(function: Obj<Function>, super_init: Option<Obj<Closure>>) -> Self {
        Self {
            class: function,
            upvalues: Vec::new(),
        }
    }
}

impl Traceable for Closure {
    const KIND: u8 = 4;
    fn trace(&self, collector: &mut Vec<Handle>) {
        collector.push(self.class.downgrade());
        for upvalue in self.upvalues.iter() {
            collector.push(upvalue.downgrade());
        }
    }
}

pub struct Instance {
    class: Obj<Class>,
    properties: HashMap<String, Value>,
}

impl Traceable for Instance {
    const KIND: u8 = 5;

    fn trace(&self, collector: &mut Vec<Handle>) {
        for value in self.properties.values() {
            if let Value::Obj(handle) = value {
                collector.push(*handle)
            }
        }
    }
}

impl Instance {
    pub fn new(class: Obj<Class>) -> Self {
        Self {
            class,
            properties: HashMap::new(),
        }
    }
}

pub struct BoundMethod {
    receiver: Obj<Instance>,
    method: Obj<Closure>,
}

impl BoundMethod {
    pub fn new(receiver: Obj<Instance>, method: Obj<Closure>) -> Self {
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
