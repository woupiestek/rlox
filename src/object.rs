// run time data structures

use std::{
    collections::HashMap,
    fmt::Display,
    hash::{Hash, Hasher},
};

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

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        if value {
            Value::True
        } else {
            Value::False
        }
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Number(value)
    }
}

impl<T: Traceable> From<Obj<T>> for Value {
    fn from(value: Obj<T>) -> Self {
        Value::Object(Handle::from(value))
    }
}

impl Value {
    pub fn is_falsey(&self) -> bool {
        match self {
            Value::Nil | Value::False => true,
            _ => false,
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::False => write!(f, "false"),
            Value::Nil => write!(f, "nil"),
            Value::Number(a) => write!(f, "{}", a),
            Value::Object(a) => write!(f, "{}", a),
            Value::True => write!(f, "true"),
        }
    }
}

impl PartialEq for Obj<String> {
    fn eq(&self, other: &Self) -> bool {
        self.as_ptr() == other.as_ptr() || **self == **other
    }
}

impl Eq for Obj<String> {}

impl Hash for Obj<String> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_str(self).hash(state);
    }
}

impl Traceable for String {
    const KIND: Kind = Kind::String;
}
pub fn hash_str(chars: &str) -> u32 {
    let mut hash = 2166136261u32;
    for &byte in chars.as_bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    return hash;
}

pub struct Function {
    pub name: Option<Obj<String>>,
    pub arity: u8,
    pub upvalue_count: u8,
    pub chunk: Chunk,
}

impl Function {
    pub fn new(name: Option<Obj<String>>) -> Self {
        Self {
            name,
            arity: 0,
            upvalue_count: 0,
            chunk: Chunk::new(),
        }
    }
}

impl Display for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(str) = self.name {
            write!(f, "<fn {}({}/{})>", *str, self.arity, self.upvalue_count)
        } else {
            write!(f, "<script>")
        }
    }
}

impl Traceable for Function {
    const KIND: Kind = Kind::Function;
}

pub struct Class {
    pub name: Obj<String>,
    pub methods: HashMap<Obj<String>, Obj<Closure>>,
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
}

pub enum Upvalue {
    Open(usize, Option<Obj<Upvalue>>),
    Closed(Value),
}

impl Traceable for Upvalue {
    const KIND: Kind = Kind::Upvalue;
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
}

pub struct Instance {
    pub class: Obj<Class>,
    pub properties: HashMap<Obj<String>, Value>,
}

impl Traceable for Instance {
    const KIND: Kind = Kind::Instance;
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
    pub receiver: Obj<Instance>,
    pub method: Obj<Closure>,
}

impl BoundMethod {
    pub fn new(receiver: Obj<Instance>, method: Obj<Closure>) -> Self {
        Self { receiver, method }
    }
}

impl Traceable for BoundMethod {
    const KIND: Kind = Kind::BoundMethod;
}

// perhaps Native should
#[derive(Copy, Clone)]
pub struct Native(pub fn(args: &[Value]) -> Result<Value, String>);

impl std::fmt::Debug for Native {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<native function>")
    }
}

impl Traceable for Native {
    const KIND: Kind = Kind::Native;
}

pub trait ObjVisitor<T> {
    fn visit_bound_method(&mut self, obj: Obj<BoundMethod>) -> T;
    fn visit_class(&mut self, obj: Obj<Class>) -> T;
    fn visit_closure(&mut self, obj: Obj<Closure>) -> T;
    fn visit_function(&mut self, obj: Obj<Function>) -> T;
    fn visit_instance(&mut self, obj: Obj<Instance>) -> T;
    fn visit_native(&mut self, obj: Obj<Native>) -> T;
    fn visit_string(&mut self, obj: Obj<String>) -> T;
    fn visit_upvalue(&mut self, obj: Obj<Upvalue>) -> T;
}
