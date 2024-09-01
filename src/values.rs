// run time data structures

use crate::{
    bound_methods::BoundMethodHandle,
    classes::ClassHandle,
    closures::ClosureHandle,
    functions::FunctionHandle,
    heap::{Collector, Heap},
    instances::InstanceHandle,
    natives::NativeHandle,
    strings::StringHandle,
};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Value {
    Nil,
    True,
    False,
    Function(FunctionHandle),
    Native(NativeHandle),
    Number(f64),
    BoundMethod(BoundMethodHandle),
    String(StringHandle),
    StackRef(u16), // for open upvalues
    Closure(ClosureHandle),
    Class(ClassHandle),
    Instance(InstanceHandle),
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

impl Value {
    pub fn is_falsey(&self) -> bool {
        matches!(self, Value::Nil | Value::False)
    }

    pub fn as_class(&self) -> Result<ClassHandle, String> {
        if let &Value::Class(handle) = self {
            Ok(handle)
        } else {
            err!("Not a class")
        }
    }

    pub fn as_function(&self) -> Result<FunctionHandle, String> {
        if let &Value::Function(handle) = self {
            Ok(handle)
        } else {
            err!("Not a function")
        }
    }

    pub fn as_instance(&self) -> Result<InstanceHandle, String> {
        if let &Value::Instance(handle) = self {
            Ok(handle)
        } else {
            err!("Not a class")
        }
    }

    pub fn to_string(&self, heap: &Heap) -> String {
        match self {
            Value::False => format!("false"),
            Value::Nil => format!("nil"),
            Value::Number(a) => format!("{}", a),
            Value::BoundMethod(a) => heap.bound_methods.to_string(*a, heap),
            Value::True => format!("true"),
            Value::Native(_) => format!("<native function>"),
            Value::String(a) => heap.strings.get(*a).unwrap().to_owned(),
            Value::Function(a) => heap.functions.to_string(*a, heap),
            Value::StackRef(i) => format!("&{}", i),
            Value::Closure(a) => heap
                .functions
                .to_string(heap.closures.function_handle(*a), heap),
            Value::Class(a) => heap.classes.to_string(*a, &heap.strings),
            Value::Instance(a) => heap.instances.to_string(*a, heap),
        }
    }

    pub fn trace(&self, collector: &mut Collector) {
        match self {
            Value::BoundMethod(h) => collector.push(*h),
            Value::String(h) => collector.push(*h),
            Value::Closure(h) => collector.push(*h),
            Value::Class(h) => collector.push(*h),
            Value::Instance(h) => collector.push(*h),
            // Value::Function(_) => todo!(),
            // Value::Native(_) => todo!(),
            _ => (),
        }
    }
}
