// run time data structures

use crate::{
    classes::ClassHandle,
    closures::ClosureHandle,
    functions::{FunctionHandle, Functions},
    heap::{Collector, Heap, Kind, ObjectHandle, Traceable},
    natives::NativeHandle,
    strings::{Map, StringHandle},
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
    Closure(ClosureHandle),
    Class(ClassHandle),
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

    pub fn as_class(&self) -> Result<ClassHandle, String> {
        if let &Value::Class(handle) = self {
            Ok(handle)
        } else {
            err!("Not an object")
        }
    }

    pub fn as_function(&self) -> Result<FunctionHandle, String> {
        if let &Value::Function(handle) = self {
            Ok(handle)
        } else {
            err!("Not a function")
        }
    }

    pub fn to_string(&self, heap: &Heap, functions: &Functions) -> String {
        match self {
            Value::False => format!("false"),
            Value::Nil => format!("nil"),
            Value::Number(a) => format!("{}", a),
            Value::Object(a) => heap.to_string(*a, functions),
            Value::True => format!("true"),
            Value::Native(_) => format!("<native function>"),
            Value::String(a) => heap.get_str(*a).to_owned(),
            Value::Function(a) => functions.to_string(*a, heap),
            Value::StackRef(i) => format!("&{}", i),
            Value::Closure(a) => functions.to_string(heap.closures.function_handle(*a), heap),
            Value::Class(a) => heap.classes.to_string(*a, &heap.strings),
        }
    }

    pub fn trace(&self, collector: &mut Collector) {
        match self {
            Value::Object(h) => collector.push(*h),
            Value::String(h) => collector.push(*h),
            Value::Closure(h) => collector.push(*h),
            Value::Class(h) => collector.push(*h),
            // Value::Function(_) => todo!(),
            // Value::Native(_) => todo!(),
            _ => (),
        }
    }
}

pub struct Instance {
    pub class: ClassHandle,
    // heap allocated
    pub properties: Map<Value>,
}

impl Traceable for Instance {
    const KIND: Kind = Kind::Instance;

    fn byte_count(&self) -> usize {
        40 + 24 * self.properties.capacity()
    }

    fn trace(&self, collector: &mut Collector) {
        collector.push(self.class);
        self.properties.trace(collector);
    }
}

impl Instance {
    pub fn new(class: ClassHandle) -> Self {
        Self {
            class,
            properties: Map::new(),
        }
    }
}

pub struct BoundMethod {
    pub receiver: ObjectHandle,
    pub method: ClosureHandle,
}

impl BoundMethod {
    pub fn new(receiver: ObjectHandle, method: ClosureHandle) -> Self {
        Self { receiver, method }
    }
}

impl Traceable for BoundMethod {
    const KIND: Kind = Kind::BoundMethod;

    fn byte_count(&self) -> usize {
        16
    }

    fn trace(&self, collector: &mut Collector) {
        collector.push(ObjectHandle::from(self.receiver));
        collector.push(self.method);
    }
}
