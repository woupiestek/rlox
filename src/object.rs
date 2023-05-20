// run time data structures

use std::collections::HashMap;

use crate::{
    class::{Class, Method, Symbol},
    memory::{Handle, Kind, Traceable, TypedHandle},
};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Value {
    Nil,
    True,
    False,
    Number(f64),
    Obj(Handle),
    Native(NativeFn),
}

#[derive(Copy, Clone)]
pub struct NativeFn(fn(args: &[Value]) -> Value);
impl std::fmt::Debug for NativeFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<native function>")
    }
}
impl PartialEq for NativeFn {
    fn eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl Value {
    pub fn mark(&mut self, gray: &mut Vec<Handle>) {
        if let Value::Obj(mut handle) = self {
            handle.mark(true);
            gray.push(handle);
        }
    }
}

pub struct Upvalue {
    location: *mut Value,
    closed: Option<Value>,
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
    const KIND: Kind = Kind::Constructor;

    fn trace(&self, collector: &mut Vec<Handle>) {
        for upvalue in self.upvalues.iter() {
            // upvalue not yet the right type
        }
    }
}

pub struct Instance {
    constructor: TypedHandle<Constructor>,
    fields: HashMap<Symbol, Value>,
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

pub enum RuntimeError {
    TypeMismatch,
    ArityMismatch,
    StackOverflow,
    OutOfMemory,
    FieldNotFound,
}
