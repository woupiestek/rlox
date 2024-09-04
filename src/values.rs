// run time data structures

use crate::heap::{
    Collector, Handle, Heap, BOUND_METHOD, CLASS, CLOSURE, FUNCTION, INSTANCE, NATIVE, STRING,
};

// #[derive(Copy, Clone, Debug, PartialEq)]
// pub enum Value {
//     Nil,
//     True,
//     False,
//     Function(FunctionHandle),
//     Native(NativeHandle),
//     Number(f64),
//     BoundMethod(BoundMethodHandle),
//     String(StringHandle),
//     StackRef(u16), // for open upvalues
//     Closure(ClosureHandle),
//     Class(ClassHandle),
//     Instance(InstanceHandle),
// }

// impl From<StringHandle> for Value {
//     fn from(value: StringHandle) -> Self {
//         Self::String(value)
//     }
// }

// impl From<FunctionHandle> for Value {
//     fn from(value: FunctionHandle) -> Self {
//         Self::Function(value)
//     }
// }

// impl From<bool> for Value {
//     fn from(value: bool) -> Self {
//         if value {
//             Value::True
//         } else {
//             Value::False
//         }
//     }
// }

// impl From<f64> for Value {
//     fn from(value: f64) -> Self {
//         Value::Number(value)
//     }
// }

// impl Value {
//     pub fn is_falsey(&self) -> bool {
//         matches!(self, Value::Nil | Value::False)
//     }

//     pub fn as_class(&self) -> Result<ClassHandle, String> {
//         if let &Value::Class(handle) = self {
//             Ok(handle)
//         } else {
//             err!("Not a class")
//         }
//     }

//     pub fn as_function(&self) -> Result<FunctionHandle, String> {
//         if let &Value::Function(handle) = self {
//             Ok(handle)
//         } else {
//             err!("Not a function")
//         }
//     }

//     pub fn as_instance(&self) -> Result<InstanceHandle, String> {
//         if let &Value::Instance(handle) = self {
//             Ok(handle)
//         } else {
//             err!("Not a class")
//         }
//     }

//     pub fn to_string(&self, heap: &Heap) -> String {
//         match self {
//             Value::False => format!("false"),
//             Value::Nil => format!("nil"),
//             Value::Number(a) => format!("{}", a),
//             Value::BoundMethod(a) => heap.bound_methods.to_string(*a, heap),
//             Value::True => format!("true"),
//             Value::Native(_) => format!("<native function>"),
//             Value::String(a) => heap.strings.get(*a).unwrap().to_owned(),
//             Value::Function(a) => heap.functions.to_string(*a, heap),
//             Value::StackRef(i) => format!("&{}", i),
//             Value::Closure(a) => heap
//                 .functions
//                 .to_string(heap.closures.function_handle(*a), heap),
//             Value::Class(a) => heap.classes.to_string(*a, &heap.strings),
//             Value::Instance(a) => heap.instances.to_string(*a, heap),
//         }
//     }

//     pub fn trace(&self, collector: &mut Collector) {
//         match self {
//             Value::BoundMethod(h) => collector.push(*h),
//             Value::String(h) => collector.push(*h),
//             Value::Closure(h) => collector.push(*h),
//             Value::Class(h) => collector.push(*h),
//             Value::Instance(h) => collector.push(*h),
//             // Value::Function(_) => todo!(),
//             // Value::Native(_) => todo!(),
//             _ => (),
//         }
//     }
// }

// nan box?
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Value(u64);

const QNAN: u64 = 0x7ffc_0000_0000_0000;

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self(value.to_bits())
    }
}

impl TryFrom<Value> for f64 {
    type Error = String;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if value.is_number() {
            Ok(f64::from_bits(value.0))
        } else {
            err!("value is not a number")
        }
    }
}

impl<const KIND: usize> From<Handle<KIND>> for Value {
    fn from(value: Handle<KIND>) -> Self {
        Self(0xfffc_0000_0000_0000 | value.0 as u64 | (KIND << 32) as u64)
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        if value {
            Self::TRUE
        } else {
            Self::FALSE
        }
    }
}

impl<const KIND: usize> TryFrom<Value> for Handle<KIND> {
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if value.0 >> 32 == 0xfffc_0000 | KIND as u64 {
            Ok(Handle::from((value.0 & 0xffff_ffff) as u32))
        } else {
            err!("Value is no handle")
        }
    }

    type Error = String;
}

impl Value {
    pub fn is_number(&self) -> bool {
        self.0 & QNAN != QNAN
    }

    // nil, true, false, stack_ref
    pub const NIL: Self = Self(QNAN | 1);
    pub const TRUE: Self = Self(QNAN | 2);
    pub const FALSE: Self = Self(QNAN | 3);

    pub fn is_falsey(&self) -> bool {
        matches!(self, &Value::NIL | &Value::FALSE)
    }

    pub fn from_stack_ref(index: u16) -> Self {
        Self(0x7ffc_0000_0001_0000 | (index as u64))
    }

    pub fn as_stack_ref(&self) -> Option<usize> {
        if 0x7ffc_0000_0001_0000 & self.0 == 0x7ffc_0000_0001_0000 {
            Some((self.0 & 0xffff) as usize)
        } else {
            None
        }
    }

    pub fn kind(&self) -> Option<usize> {
        if self.0 & 0xffff_0000_0000_0000 != 0xfffc_0000_0000_0000 {
            return None;
        }
        return Some(((self.0 >> 32) & 0xffff) as usize);
    }

    pub fn trace(&self, collector: &mut Collector) {
        if 0xffff_0000_0000_0000 & self.0 != 0xfffc_0000_0000_0000 {
            return;
        }
        let index = (self.0 & 0xffff_ffff) as u32;
        match (self.0 >> 32 & 0xffff) as usize {
            BOUND_METHOD => collector.push(Handle::<BOUND_METHOD>::from(index)),
            CLASS => collector.push(Handle::<CLASS>::from(index)),
            CLOSURE => collector.push(Handle::<CLOSURE>::from(index)),
            INSTANCE => collector.push(Handle::<INSTANCE>::from(index)),
            STRING => collector.push(Handle::<STRING>::from(index)),
            _ => (),
        }
    }

    pub fn to_string(&self, heap: &Heap) -> String {
        match self {
            &Value::FALSE => return format!("false"),
            &Value::NIL => return format!("nil"),
            &Value::TRUE => return format!("true"),
            _ => (),
        }

        if self.0 & QNAN != QNAN {
            return format!("{}", f64::from_bits(self.0));
        }

        if 0x8000_0000_0000_0000 & self.0 == 0x8000_0000_0000_0000 {
            let index = (self.0 & 0xffff_ffff) as u32;
            match ((self.0 >> 32) & 0x000f) as usize {
                BOUND_METHOD => return heap.bound_methods.to_string(Handle::from(index), heap),
                CLASS => return heap.classes.to_string(Handle::from(index), &heap.strings),
                CLOSURE => {
                    return heap
                        .functions
                        .to_string(heap.closures.function_handle(Handle::from(index)), heap)
                }
                INSTANCE => return heap.instances.to_string(Handle::from(index), heap),
                STRING => return heap.strings.get(Handle::from(index)).unwrap().to_owned(),
                FUNCTION => {
                    return heap.functions.to_string(Handle::from(index), heap);
                }
                NATIVE => return format!("<native function>"),
                _ => (),
            }
        }

        if 0x7ffc_0000_0001_0000 & self.0 == 0x7ffc_0000_0001_0000 {
            return format!("&{}", self.0 & 0xffff);
        }

        format!("<invalid {:#x}>", self.0)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    pub fn handling_numbers() {
        let value = Value::from(1.23456789);
        assert!(value.is_number());
        assert_eq!(f64::try_from(value), Ok(1.23456789));
        assert!(Handle::<3>::try_from(value).is_err());
    }

    #[test]
    pub fn handling_handles() {
        let value = Value::from(Handle::<7>::from(123456789));
        assert_eq!(
            Handle::<7>::try_from(value),
            Ok(Handle::<7>::from(123456789))
        );
        assert!(Handle::<3>::try_from(value).is_err());
        assert!(f64::try_from(value).is_err());
        assert!(!value.is_falsey());
    }

    #[test]
    pub fn handling_nil() {
        let value = Value::NIL;
        assert!(value.is_falsey());
        assert!(f64::try_from(value).is_err());
        assert!(Handle::<3>::try_from(value).is_err());
    }

    #[test]
    pub fn handling_booleans() {
        assert!(Value::FALSE.is_falsey());
        assert!(!Value::TRUE.is_falsey());
        assert!(f64::try_from(Value::FALSE).is_err());
        assert!(Handle::<3>::try_from(Value::TRUE).is_err());
    }
}
