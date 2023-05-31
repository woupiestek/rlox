// run time data structures

use std::{collections::HashMap, fmt::Display};

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
        Value::Object(value.as_handle())
    }
}

impl Value {
    pub fn is_falsey(&self) -> bool {
        match self {
            Value::Nil | Value::False => true,
            _ => false,
        }
    }
    pub fn type_name(&self) -> &str {
        match self {
            Value::False => "boolean",
            Value::Nil => "nil",
            Value::Number(_) => "number",
            Value::Object(a) => match a.kind() {
                Kind::BoundMethod => "bound_method",
                Kind::Class => "class",
                Kind::Closure => "closure",
                Kind::Function => "function",
                Kind::Instance => "instance",
                Kind::Native => "native",
                Kind::String => "string",
                Kind::Upvalue => "upvalue",
            },
            Value::True => "boolean",
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

    fn trace(&self, collector: &mut Vec<Handle>) {
        if let Some(n) = &self.name {
            collector.push(n.as_handle())
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
    fn trace(&self, collector: &mut Vec<Handle>) {
        collector.push(self.name.as_handle());
        for (name, method) in &self.methods {
            collector.push(name.as_handle());
            collector.push(method.as_handle());
        }
    }
}

pub enum Upvalue {
    Open(usize, Option<Obj<Upvalue>>),
    Closed(Value),
}

impl Traceable for Upvalue {
    const KIND: Kind = Kind::Upvalue;

    fn trace(&self, collector: &mut Vec<Handle>) {
        match self {
            Upvalue::Open(_, Some(next)) => collector.push(next.as_handle()),
            Upvalue::Closed(Value::Object(handle)) => collector.push(*handle),
            _ => (),
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
        collector.push(self.function.as_handle());
        for upvalue in self.upvalues.iter() {
            collector.push(upvalue.as_handle());
        }
    }
}

pub struct Instance {
    pub class: Obj<Class>,
    pub properties: HashMap<Obj<String>, Value>,
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

    fn trace(&self, collector: &mut Vec<Handle>) {
        collector.push(self.receiver.as_handle());
        collector.push(self.method.as_handle());
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
