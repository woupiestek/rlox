use crate::object::Value;

pub struct Natives(Vec<fn(args: &[Value]) -> Result<Value, String>>);

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct NativeHandle {
    index: u8, // More than enough for now...
}

// All natives are collected on shut down.
impl Natives {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn store(&mut self, f: fn(args: &[Value]) -> Result<Value, String>) -> NativeHandle {
        let index = self.0.len();
        self.0.push(f);
        NativeHandle { index: index as u8 }
    }

    pub fn call(&self, handle: NativeHandle, args: &[Value]) -> Result<Value, String> {
        self.0[handle.index as usize](args)
    }
}


#[cfg(test)]
mod tests {
    use crate::object::Value;

    use super::*;

    #[test]
    fn no_stack_overflow_on_init() {
        Natives::new();
    }

    fn first(_args: &[Value]) -> Result<Value, String> {
        if _args.len() > 0 {
            Ok(_args[0])
        } else {
            err!("Too few arguments.")
        }
    }

    #[test]
    fn store_native_function() {
        let mut natives = Natives::new();
        let handle = natives.store(first);
        assert_eq!(natives.call(handle,&[Value::Nil]),Ok(Value::Nil));
    }
}

