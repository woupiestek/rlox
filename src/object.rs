// run time data structures

use crate::{
    chunk::Chunk,
    heap::{Handle, Heap, Kind, Traceable},
    natives::NativeHandle,
    strings::{KeySet, Map, StringHandle},
};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Value {
    Nil,
    True,
    False,
    Native(NativeHandle),
    Number(f64),
    Object(Handle),
    String(StringHandle),
}

impl From<StringHandle> for Value {
    fn from(value: StringHandle) -> Self {
        Self::String(value)
    }
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
            Value::String(a) => heap.get_str(*a).to_owned(),
        }
    }
}

pub struct Function {
    pub name: Option<StringHandle>,
    pub arity: u8,
    pub upvalue_count: u8,
    pub chunk: Chunk,
}

impl Function {
    pub fn new(name: Option<StringHandle>) -> Self {
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

    fn trace(&self, collector: &mut Vec<Handle>, key_set: &mut KeySet) {
        if let Some(name) = self.name {
            key_set.put(name);
        }
        for &value in &self.chunk.constants {
            if let Value::Object(h) = value {
                collector.push(h)
            }
        }
    }
}

pub struct Class {
    pub name: StringHandle,
    // heap allocated
    pub methods: Map<Handle>,
}

impl Class {
    pub fn new(name: StringHandle) -> Self {
        Self {
            name,
            methods: Map::new(),
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

    fn trace(&self, collector: &mut Vec<Handle>, key_set: &mut KeySet) {
        key_set.put(self.name);
        self.methods.trace(collector, key_set);
    }
}

pub enum Upvalue {
    Open(usize, Option<Handle>),
    // store any value on the heap...
    // allow this value to change into other types of value
    Closed(Value),
}

impl Traceable for Upvalue {
    const KIND: Kind = Kind::Upvalue;

    fn byte_count(&self) -> usize {
        24
    }

    fn trace(&self, collector: &mut Vec<Handle>, key_set: &mut KeySet) {
        match *self {
            Upvalue::Open(_, Some(next)) => collector.push(Handle::from(next)),
            Upvalue::Closed(Value::Object(handle)) => collector.push(handle),
            Upvalue::Closed(Value::String(handle)) => key_set.put(handle),
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

    fn trace(&self, collector: &mut Vec<Handle>, _key_set: &mut KeySet) {
        collector.push(Handle::from(self.function));
        for &upvalue in self.upvalues.iter() {
            collector.push(Handle::from(upvalue));
        }
    }
}

pub struct Instance {
    pub class: Handle,
    // heap allocated
    pub properties: Map<Value>,
}

impl Traceable for Instance {
    const KIND: Kind = Kind::Instance;

    fn byte_count(&self) -> usize {
        40 + 24 * self.properties.capacity()
    }

    fn trace(&self, collector: &mut Vec<Handle>, key_set: &mut KeySet) {
        collector.push(Handle::from(self.class));
        self.properties.trace(collector, key_set);
    }
}

impl Instance {
    pub fn new(class: Handle) -> Self {
        Self {
            class,
            properties: Map::new(),
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

    fn trace(&self, collector: &mut Vec<Handle>, _key_set: &mut KeySet) {
        collector.push(Handle::from(self.receiver));
        collector.push(Handle::from(self.method));
    }
}
