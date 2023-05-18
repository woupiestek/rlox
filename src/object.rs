// run time data structures

use std::collections::HashMap;

use crate::class::{Class, Method, Path, Symbol};

pub struct Handle<T> {
    value: *mut (bool, T),
}

// just like that !?
impl<T> Copy for Handle<T> {}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
        }
    }
}

#[derive(Copy, Clone)]
pub enum Value {
    Nil,
    True,
    False,
    Number(f64),
    Constructor(Handle<Constructor>),
    Instance(Handle<Instance>),
    BoundMethod(Handle<(Instance, Path<Method>)>),
    Native(Handle<fn(args: &[Value]) -> Value>),
    String(Handle<String>),
    Upvalue(Handle<Upvalue>),
}

pub struct Upvalue {
    location: usize,
    closed: Option<Value>,
}

pub struct Constructor {
    class: Path<Class>,
    up_values: Vec<Upvalue>,
}

pub struct Instance {
    constructor: Handle<Constructor>,
    fields: HashMap<Symbol, Value>,
}

pub enum RuntimeError {
    ArityMismatch,
    StackOverflow,
    OutOfMemory,
    FieldNotFound,
}
