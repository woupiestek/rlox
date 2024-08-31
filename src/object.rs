// run time data structures

use crate::{
    functions::{FunctionHandle, Functions},
    heap::{Collector, Heap, Kind, ObjectHandle, Traceable},
    natives::NativeHandle,
    strings::{Map, StringHandle}, upvalues::UpvalueHandle,
};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Value {
    Nil,
    True,
    False,
    Function(FunctionHandle),
    Native(NativeHandle),
    Number(f64),
    Object(ObjectHandle),
    String(StringHandle),
    StackRef(u16), // for open upvalues
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

impl From<ObjectHandle> for Value {
    fn from(value: ObjectHandle) -> Self {
        Value::Object(ObjectHandle::from(value))
    }
}

impl Value {
    pub fn is_falsey(&self) -> bool {
        matches!(self, Value::Nil | Value::False)
    }

    pub fn as_object(&self) -> Result<ObjectHandle, String> {
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

    pub fn to_string(&self, heap: &Heap, byte_code: &Functions) -> String {
        match self {
            Value::False => format!("false"),
            Value::Nil => format!("nil"),
            Value::Number(a) => format!("{}", a),
            Value::Object(a) => heap.to_string(*a, byte_code),
            Value::True => format!("true"),
            Value::Native(_) => format!("<native function>"),
            Value::String(a) => heap.get_str(*a).to_owned(),
            Value::Function(a) => byte_code.to_string(*a, heap),
            Value::StackRef(i) => format!("&{}",i),
        }
    }
}

pub struct Class {
    pub name: StringHandle,
    // heap allocated
    pub methods: Map<ObjectHandle>,
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

    fn trace(&self, collector: &mut Collector) {
        collector.strings.push(self.name);
        self.methods.trace(collector);
    }
}

// I guess the constructor can own the upvalues,
// though the class basically already determines how many are needed.
pub struct Closure {
    pub function: FunctionHandle,
    // heap allocated
    pub upvalues: Vec<UpvalueHandle>,
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

    fn trace(&self, collector: &mut Collector) {
        for &upvalue in self.upvalues.iter() {
            collector.upvalues.push(upvalue);
        }
    }
}

pub struct Instance {
    pub class: ObjectHandle,
    // heap allocated
    pub properties: Map<Value>,
}

impl Traceable for Instance {
    const KIND: Kind = Kind::Instance;

    fn byte_count(&self) -> usize {
        40 + 24 * self.properties.capacity()
    }

    fn trace(&self, collector: &mut Collector) {
        collector.objects.push(ObjectHandle::from(self.class));
        self.properties.trace(collector);
    }
}

impl Instance {
    pub fn new(class: ObjectHandle) -> Self {
        Self {
            class,
            properties: Map::new(),
        }
    }
}

pub struct BoundMethod {
    pub receiver: ObjectHandle,
    pub method: ObjectHandle,
}

impl BoundMethod {
    pub fn new(receiver: ObjectHandle, method: ObjectHandle) -> Self {
        Self { receiver, method }
    }
}

impl Traceable for BoundMethod {
    const KIND: Kind = Kind::BoundMethod;

    fn byte_count(&self) -> usize {
        16
    }

    fn trace(&self, collector: &mut Collector) {
        collector.objects.push(ObjectHandle::from(self.receiver));
        collector.objects.push(ObjectHandle::from(self.method));
    }
}
