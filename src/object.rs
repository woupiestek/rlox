// run time data structures

use std::fmt::Display;

use crate::{
    chunk::Chunk,
    loxtr::Loxtr,
    memory::{Handle, Kind, Traceable, GC},
    table::Table,
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

impl<T: Traceable> From<GC<T>> for Value {
    fn from(value: GC<T>) -> Self {
        Value::Object(Handle::from(value))
    }
}

impl Value {
    pub fn is_falsey(&self) -> bool {
        matches!(self, Value::Nil | Value::False)
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::False => write!(f, "false"),
            Value::Nil => write!(f, "nil"),
            Value::Number(a) => a.fmt(f),
            Value::Object(a) => a.fmt(f),
            Value::True => write!(f, "true"),
        }
    }
}

impl Traceable for Loxtr {
    const KIND: Kind = Kind::String;
    fn byte_count(&self) -> usize {
        self.as_ref().len() + 24
    }

    fn trace(&self, _collector: &mut Vec<Handle>) {}
}

pub struct Function {
    pub name: Option<GC<Loxtr>>,
    pub arity: u8,
    pub upvalue_count: u8,
    pub chunk: Chunk,
}

impl Function {
    pub fn new(name: Option<GC<Loxtr>>) -> Self {
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
    pub name: GC<Loxtr>,
    // heap allocated
    pub methods: Table<GC<Closure>>,
}

impl Class {
    pub fn new(name: GC<Loxtr>) -> Self {
        Self {
            name,
            methods: Table::new(),
        }
    }
}

impl Traceable for Class {
    const KIND: Kind = Kind::Class;

    fn byte_count(&self) -> usize {
        // 32 is 8 for name and 32 for Table
        // 16 is 8 for obj, 8 for closure
        40 + 16 * self.methods.capacity()
    }

    fn trace(&self, collector: &mut Vec<Handle>) {
        collector.push(Handle::from(self.name));
        self.methods.trace(collector);
    }
}

impl Display for Class {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<class {}>", *self.name)
    }
}

pub enum Upvalue {
    Open(usize, Option<GC<Upvalue>>),
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
    pub function: GC<Function>,
    // heap allocated
    pub upvalues: Vec<GC<Upvalue>>,
}

impl Closure {
    pub fn new(function: GC<Function>) -> Self {
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
    pub class: GC<Class>,
    // heap allocated
    pub properties: Table<Value>,
}

impl Traceable for Instance {
    const KIND: Kind = Kind::Instance;

    fn byte_count(&self) -> usize {
        40 + 24 * self.properties.capacity()
    }

    fn trace(&self, collector: &mut Vec<Handle>) {
        collector.push(Handle::from(self.class));
        self.properties.trace(collector);
    }
}

impl Instance {
    pub fn new(class: GC<Class>) -> Self {
        Self {
            class,
            properties: Table::new(),
        }
    }
}

impl Display for Instance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<{} instance>", *self.class)
    }
}
pub struct BoundMethod {
    pub receiver: GC<Instance>,
    pub method: GC<Closure>,
}

impl BoundMethod {
    pub fn new(receiver: GC<Instance>, method: GC<Closure>) -> Self {
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
