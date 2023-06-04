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
    fn byte_count(&self) -> usize {
        self.capacity() + 24
    }

    fn trace(&self, _collector: &mut Vec<Handle>) {}
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
    // just consider initial allocation
    fn byte_count(&self) -> usize {
        60 + self.chunk.byte_increment()
    }

    fn trace(&self, collector: &mut Vec<Handle>) {
        if let Some(name) = self.name {
            collector.push(Handle::from(name))
        }
        for &value in &self.chunk.constants {
            if let Value::Object(h) = value {
                collector.push(h)
            }
        }
    }
}

pub struct Class {
    pub name: Obj<String>,
    // heap allocated
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

    fn byte_count(&self) -> usize {
        // 32 is 8 for name and 24 for hashmap, assuming similar size to Vec
        // 36 is 8 for obj, 16 for value, 8 for hash, +10% for scattering
        32 + 36 * self.methods.capacity()
    }

    fn trace(&self, collector: &mut Vec<Handle>) {
        collector.push(Handle::from(self.name));
        for (name, method) in &self.methods {
            collector.push(Handle::from(*name));
            collector.push(Handle::from(*method));
        }
    }
}

impl Display for Class {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<class {}>", *self.name)
    }
}

pub enum Upvalue {
    Open(usize, Option<Obj<Upvalue>>),
    Closed(Value),
}

impl Traceable for Upvalue {
    const KIND: Kind = Kind::Upvalue;

    fn byte_count(&self) -> usize {
        24
    }

    fn trace(&self, collector: &mut Vec<Handle>) {
        match *self {
            Upvalue::Open(_, Some(next)) => collector.push(Handle::from(next)),
            Upvalue::Closed(Value::Object(handle)) => collector.push(handle),
            _ => (),
        }
    }
}

impl Display for Upvalue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<upvalue>")
    }
}

// I guess the constructor can own the upvalues,
// though the class basically already determines how many are needed.
pub struct Closure {
    pub function: Obj<Function>,
    // heap allocated
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

    fn byte_count(&self) -> usize {
        16 + self.upvalues.capacity()
    }

    fn trace(&self, collector: &mut Vec<Handle>) {
        collector.push(Handle::from(self.function));
        for upvalue in self.upvalues.iter() {
            collector.push(Handle::from(*upvalue));
        }
    }
}
impl Display for Closure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.function.fmt(f)
    }
}

pub struct Instance {
    pub class: Obj<Class>,
    // heap allocated
    pub properties: HashMap<Obj<String>, Value>,
}

impl Traceable for Instance {
    const KIND: Kind = Kind::Instance;

    fn byte_count(&self) -> usize {
        // 32 is 8 for class and 24 for hashmap, assuming similar size to Vec
        // 36 is 8 for obj, 16 for value, 8 for hash, +10% for scattering
        32 + 36 * self.properties.capacity()
    }

    fn trace(&self, collector: &mut Vec<Handle>) {
        collector.push(Handle::from(self.class));
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

impl Display for Instance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<{} instance>", *self.class)
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

    fn byte_count(&self) -> usize {
        16
    }

    fn trace(&self, collector: &mut Vec<Handle>) {
        collector.push(Handle::from(self.receiver));
        collector.push(Handle::from(self.method));
    }
}
impl Display for BoundMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.method.fmt(f)
    }
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

    fn byte_count(&self) -> usize {
        8
    }

    fn trace(&self, _collector: &mut Vec<Handle>) {}
}

impl Display for Native {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<native>")
    }
}
