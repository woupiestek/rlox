// run time data structures

use crate::{
    byte_code::{ByteCode, FunctionHandle},  heap::{Handle, Heap, Kind, Traceable}, natives::NativeHandle, strings::{Map, StringHandle}
};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Value {
    Nil,
    True,
    False,
    Function(FunctionHandle),
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

impl From<FunctionHandle> for Value {
    fn from(value: FunctionHandle) -> Self {
        Self::Function(value)
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

    pub fn as_object(&self) -> Result<Handle, String> {
        if let &Value::Object(handle) = self {
            Ok(handle)
        } else {
            err!("Not an object")
        }
    }

    pub fn as_function(&self) -> Result<FunctionHandle, String> {
        if let &Value::Function(handle) = self {
            Ok(handle)
        } else {
            err!("Not an object")
        }
    }

    pub fn to_string(&self, heap: &Heap, byte_code: &ByteCode) -> String {
        match self {
            Value::False => format!("false"),
            Value::Nil => format!("nil"),
            Value::Number(a) => format!("{}", a),
            Value::Object(a) => heap.to_string(*a, byte_code),
            Value::True => format!("true"),
            Value::Native(_) => format!("<native function>"),
            Value::String(a) => heap.get_str(*a).to_owned(),
            Value::Function(a) => {
                let function = byte_code.function_ref(*a);
                if function.name != StringHandle::EMPTY {
                    format!(
                        "<fn {} ({}/{})>",
                        heap.get_str(function.name),
                        function.arity,
                        function.upvalue_count
                    )
                } else {
                    format!("<script>")
                }
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

    fn trace(&self, collector: &mut Vec<Handle>, strings: &mut Vec<StringHandle>) {
        strings.push(self.name);
        self.methods.trace(collector, strings);
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

    fn trace(&self, collector: &mut Vec<Handle>, strings: &mut Vec<StringHandle>) {
        match *self {
            Upvalue::Open(_, Some(next)) => collector.push(Handle::from(next)),
            Upvalue::Closed(Value::Object(handle)) => collector.push(handle),
            Upvalue::Closed(Value::String(handle)) => strings.push(handle),
            _ => (),
        }
    }
}

// I guess the constructor can own the upvalues,
// though the class basically already determines how many are needed.
pub struct Closure {
    pub function: FunctionHandle,
    // heap allocated
    pub upvalues: Vec<Handle>,
}

impl Closure {
    pub fn new(function: FunctionHandle) -> Self {
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

    fn trace(&self, collector: &mut Vec<Handle>, _strings: &mut Vec<StringHandle>) {
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

    fn trace(&self, collector: &mut Vec<Handle>, strings: &mut Vec<StringHandle>) {
        collector.push(Handle::from(self.class));
        self.properties.trace(collector, strings);
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

    fn trace(&self, collector: &mut Vec<Handle>, _strings: &mut Vec<StringHandle>) {
        collector.push(Handle::from(self.receiver));
        collector.push(Handle::from(self.method));
    }
}
