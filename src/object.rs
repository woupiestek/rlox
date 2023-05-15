// run time data structures

use std::{collections::HashMap, ptr::NonNull};

use crate::class::{Class, Method, StaticString};

pub struct Handle<T> {
    value: NonNull<(bool, T)>,
}

// if we ever feel like NaN boxing, is it still an option?
pub enum Value {
    Nil,
    True,
    False,
    Number(f64),
    Constructor(Handle<Constructor>),
    Instance(Handle<Instance>),
    BoundMethod(Handle<(Instance, Method)>),
    Native(Handle<fn(args: &[Value]) -> Value>),
    String(Handle<String>),
    Upvalue(Handle<Upvalue>),
}

pub struct Upvalue {
    location: Handle<Value>,
    closed: Option<Value>,
}

pub struct Constructor {
    class: Box<Class>,
    up_values: Vec<Upvalue>,
}

pub struct Instance {
    constructor: Handle<Constructor>,
    fields: HashMap<StaticString, Value>,
}
