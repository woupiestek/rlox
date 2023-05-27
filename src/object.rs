// run time data structures

use std::collections::HashMap;

use crate::{
    chunk::Chunk,
    memory::{Handle, Kind, Obj, Traceable},
};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Value {
    Nil,
    True,
    False,
    Number(f64),
    Object(Handle),
}

impl Traceable for String {
    const KIND: Kind = Kind::String;
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
    const KIND: Kind = Kind::Function;

    fn trace(&self, collector: &mut Vec<Handle>) {
        if let Some(n) = &self.name {
            collector.push(n.downgrade())
        }
        for value in &self.chunk.constants {
            if let Value::Object(h) = value {
                collector.push(*h)
            }
        }
    }
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
    const KIND: Kind = Kind::Class;
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
    const KIND: Kind = Kind::Upvalue;

    fn trace(&self, collector: &mut Vec<Handle>) {
        if let Some(Value::Object(handle)) = self.closed {
            collector.push(handle);
        }
    }
}

// I guess the constructor can own the upvalues,
// though the class basically already determines how many are needed.
pub struct Closure {
    pub function: Obj<Function>,
    pub upvalues: Vec<Obj<Upvalue>>,
}

impl Closure {
    pub fn new(function: Obj<Function>) -> Self {
        Self {
            function,
            upvalues: Vec::new(),
        }
    }
}

impl Traceable for Closure {
    const KIND: Kind = Kind::Closure;
    fn trace(&self, collector: &mut Vec<Handle>) {
        collector.push(self.function.downgrade());
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
    const KIND: Kind = Kind::Instance;

    fn trace(&self, collector: &mut Vec<Handle>) {
        for value in self.properties.values() {
            if let Value::Object(handle) = value {
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
    const KIND: Kind = Kind::BoundMethod;

    fn trace(&self, collector: &mut Vec<Handle>) {
        collector.push(self.receiver.downgrade());
        collector.push(self.method.downgrade());
    }
}

// perhaps Native should
#[derive(Copy, Clone)]
pub struct Native(pub fn(args: &[Value]) -> Value);

impl std::fmt::Debug for Native {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<native function>")
    }
}

impl Traceable for Native {
    const KIND: Kind = Kind::Native;

    fn trace(&self, _collector: &mut Vec<Handle>) {}
}
