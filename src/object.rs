// run time data structures

use crate::{
    chunk::Chunk,
    heap::{Handle, Heap, Kind, Traceable},
    loxtr::Loxtr,
    natives::NativeHandle,
    table::Table,
};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Value {
    Nil,
    True,
    False,
    Native(NativeHandle),
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

impl From<Handle> for Value {
    fn from(value: Handle) -> Self {
        Value::Object(Handle::from(value))
    }
}

impl Value {
    pub fn is_falsey(&self) -> bool {
        matches!(self, Value::Nil | Value::False)
    }

    pub fn to_handle(&self) -> Result<Handle, String> {
        if let &Value::Object(handle) = self {
            Ok(handle)
        } else {
            err!("Not an object")
        }
    }

    pub fn to_string(&self, heap: &Heap) -> String {
        match self {
            Value::False => format!("false"),
            Value::Nil => format!("nil"),
            Value::Number(a) => format!("{}", a),
            Value::Object(a) => heap.to_string(*a),
            Value::True => format!("true"),
            Value::Native(_) => format!("<native function>"),
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
    pub name: Option<Handle>,
    pub arity: u8,
    pub upvalue_count: u8,
    pub chunk: Chunk,
}

impl Function {
    pub fn new(name: Option<Handle>) -> Self {
        Self {
            name,
            arity: 0,
            upvalue_count: 0,
            chunk: Chunk::new(),
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
    pub name: Handle,
    // heap allocated
    pub methods: Table<Handle>,
}

impl Class {
    pub fn new(name: Handle) -> Self {
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

pub enum Upvalue {
    Open(usize, Option<Handle>),
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

// I guess the constructor can own the upvalues,
// though the class basically already determines how many are needed.
pub struct Closure {
    pub function: Handle,
    // heap allocated
    pub upvalues: Vec<Handle>,
}

impl Closure {
    pub fn new(function: Handle) -> Self {
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

pub struct Instance {
    pub class: Handle,
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
    pub fn new(class: Handle) -> Self {
        Self {
            class,
            properties: Table::new(),
        }
    }
}

pub struct BoundMethod {
    pub receiver: Handle,
    pub method: Handle,
}

impl BoundMethod {
    pub fn new(receiver: Handle, method: Handle) -> Self {
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
