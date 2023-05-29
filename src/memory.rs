use std::{
    alloc::alloc,
    alloc::Layout,
    collections::HashMap,
    fmt::Display,
    ops::{Deref, DerefMut},
    ptr,
};

use crate::object::{BoundMethod, Class, Closure, Function, Instance, Native, Upvalue, Value};

// note that usually 8 byte is allocated for this due to alignment, so plenty of space!

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Kind {
    BoundMethod,
    Class,
    Closure,
    Function,
    Instance,
    Native,
    String,
    Upvalue,
}

#[derive(Copy, Clone, Debug)]
pub struct Header {
    pub is_marked: bool,
    kind: Kind,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Handle {
    ptr: *mut Header,
}

impl Handle {
    pub fn kind(&self) -> Kind {
        unsafe { (*self.ptr).kind }
    }
    fn is_marked(&self) -> bool {
        unsafe { (*self.ptr).is_marked }
    }
    fn mark(&mut self, value: bool) {
        unsafe { (*self.ptr).is_marked = value }
    }
}

impl Display for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind() {
            Kind::BoundMethod => write!(
                f,
                "{}",
                *BoundMethod::obj_from_handle(self).unwrap().method.function
            ),
            Kind::Class => write!(f, "{}", *Class::obj_from_handle(self).unwrap().name),
            Kind::Function => write!(f, "{}", *Function::obj_from_handle(self).unwrap()),
            Kind::Closure => write!(f, "{}", *Closure::obj_from_handle(self).unwrap().function),
            Kind::Instance => write!(
                f,
                "{} instance",
                *Class::obj_from_handle(self).unwrap().name
            ),
            Kind::Native => write!(f, "<native fn>"),
            Kind::String => write!(f, "{}", String::from_handle(self).unwrap()),
            Kind::Upvalue => write!(f, "<upvalue>"),
        }
    }
}

#[derive(Eq, Hash, PartialEq)]
pub struct Obj<Body: Traceable> {
    ptr: *mut (Header, Body),
}

impl<T: Traceable> Obj<T> {
    fn is_marked(&self) -> bool {
        unsafe { (*self.ptr).0.is_marked }
    }

    pub fn as_handle(&self) -> Handle {
        Handle {
            ptr: self.ptr as *mut Header,
        }
    }
    pub fn as_value(&self) -> Value {
        Value::Object(self.as_handle())
    }
}

impl<T: Traceable> Copy for Obj<T> {}

impl<T: Traceable> Clone for Obj<T> {
    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr.clone(),
        }
    }
}

impl<T: Traceable> std::fmt::Debug for Obj<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Obj").field("ptr", &self.ptr).finish()
    }
}

impl<T: Traceable> Deref for Obj<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &(*self.ptr).1 }
    }
}

impl<T: Traceable> DerefMut for Obj<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut (*self.ptr).1 }
    }
}

pub trait Traceable
where
    Self: Sized,
{
    const KIND: Kind;
    fn trace(&self, collector: &mut Vec<Handle>);
    fn test_handle(handle: &Handle) -> bool {
        handle.kind() == Self::KIND
    }
    fn test_value(value: &Value) -> bool {
        if let Value::Object(handle) = value {
            Self::test_handle(handle)
        } else {
            false
        }
    }
    fn obj_from_handle(handle: &Handle) -> Result<Obj<Self>, String> {
        if Self::test_handle(handle) {
            Ok(Obj {
                ptr: handle.ptr as *mut (Header, Self),
            })
        } else {
            Err(format!(
                "Cannot upgrade {:?} to {:?}",
                handle.kind(),
                Self::KIND
            ))
        }
    }

    fn get(value: Value) -> Option<Obj<Self>> {
        if let Value::Object(handle) = value {
            if Self::test_handle(&handle) {
                return Some(Obj {
                    ptr: handle.ptr as *mut (Header, Self),
                });
            }
        }
        return None;
    }

    fn obj_from_value(value: Value) -> Result<Obj<Self>, String> {
        if let Value::Object(handle) = value {
            Self::obj_from_handle(&handle)
        } else {
            Err(format!("Cannot cast {:?} to {:?}", value, Self::KIND))
        }
    }

    fn from_handle(handle: &Handle) -> Result<&Self, String> {
        if Self::test_handle(handle) {
            let ptr = handle.ptr as *mut (Header, Self);
            Ok(unsafe { &(*ptr).1 })
        } else {
            Err(format!(
                "Cannot cast {:?} to {:?}",
                handle.kind(),
                Self::KIND
            ))
        }
    }

    fn from_value(value: &Value) -> Result<&Self, String> {
        if let Value::Object(handle) = value {
            Self::from_handle(&handle)
        } else {
            Err(format!("Cannot cast {:?} to {:?}", value, Self::KIND))
        }
    }
}

macro_rules! trace_handle {
    ($handle:expr,$traceable:ty, $collector:expr) => {{
        let ptr = $handle.ptr as *mut (Header, $traceable);
        unsafe {
            (*ptr).1.trace($collector);
        }
    }};
}
macro_rules! drop_handle {
    ($handle:expr,$traceable:ty) => {{
        let ptr = $handle.ptr as *mut (Header, $traceable);
        unsafe {
            ptr::drop_in_place(ptr);
        }
    }};
}

pub struct Heap {
    handles: Vec<Handle>,
    string_pool: HashMap<String, Obj<String>>,
}

impl Heap {
    pub fn new() -> Self {
        Self {
            handles: Vec::with_capacity(1 << 12),
            string_pool: HashMap::new(),
        }
    }

    pub fn count(&mut self) -> usize {
        self.handles.len()
    }

    pub fn intern(&mut self, name: &str) -> Obj<String> {
        match self.string_pool.get(name) {
            Some(obj) => *obj,
            None => {
                // note: two copies of name are stored now.
                // that can be avoided, for example by moving closer to the clox solution.
                let obj = self.store(name.to_string());
                self.string_pool.insert(name.to_string(), obj);
                obj
            }
        }
    }

    pub fn store<T: Traceable>(&mut self, t: T) -> Obj<T> {
        let typed = unsafe {
            let ptr = alloc(Layout::new::<(Header, T)>()) as *mut (Header, T);
            assert!(!ptr.is_null());
            ptr::write(
                ptr,
                (
                    Header {
                        is_marked: false,
                        kind: T::KIND,
                    },
                    t,
                ),
            );
            Obj { ptr }
        };
        self.handles.push(typed.as_handle());
        typed
    }

    // leave this to the caller
    pub fn collect_garbage(&mut self, roots: Vec<Handle>) {
        self.trace(roots);
        self.sweep();
    }

    fn drop_handle(&self, handle: Handle) {
        match handle.kind() {
            Kind::String => drop_handle!(handle, String),
            Kind::Function => drop_handle!(handle, Function),
            Kind::Upvalue => drop_handle!(handle, Upvalue),
            Kind::BoundMethod => drop_handle!(handle, BoundMethod),
            Kind::Class => drop_handle!(handle, Class),
            Kind::Closure => drop_handle!(handle, Closure),
            Kind::Instance => drop_handle!(handle, Instance),
            Kind::Native => drop_handle!(handle, Native),
        }
    }

    fn trace_handle(&self, handle: Handle, collector: &mut Vec<Handle>) {
        match handle.kind() {
            Kind::String => trace_handle!(handle, String, collector),
            Kind::Function => trace_handle!(handle, Function, collector),
            Kind::Upvalue => trace_handle!(handle, Upvalue, collector),
            Kind::BoundMethod => trace_handle!(handle, BoundMethod, collector),
            Kind::Class => trace_handle!(handle, Class, collector),
            Kind::Closure => trace_handle!(handle, Closure, collector),
            Kind::Instance => trace_handle!(handle, Instance, collector),
            Kind::Native => trace_handle!(handle, Native, collector),
        }
    }
    fn trace(&self, mut roots: Vec<Handle>) {
        while let Some(mut handle) = roots.pop() {
            if handle.is_marked() {
                continue;
            }
            handle.mark(true);
            self.trace_handle(handle, &mut roots);
        }
    }
    fn sweep(&mut self) {
        // clean up the string pool
        self.string_pool.retain(|_, v| v.is_marked());

        let mut index: usize = 0;
        while index < self.handles.len() {
            // look for dead object
            let mut dead = self.handles[index];
            if dead.is_marked() {
                index += 1;
                dead.mark(false);
                continue;
            }

            while self.handles.len() > index {
                // look for live object
                let mut live = self.handles.pop().unwrap();
                if live.is_marked() {
                    // swap
                    self.drop_handle(dead);
                    self.handles[index] = live;
                    live.mark(false);
                    index += 1;
                    break;
                }
                self.drop_handle(live);
            }
        }
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        while let Some(handle) = self.handles.pop() {
            self.drop_handle(handle)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::object::Value;

    use super::*;

    #[test]
    fn no_stack_overflow_on_init() {
        Heap::new();
    }

    #[test]
    fn store_empty_string() {
        let mut heap = Heap::new();
        heap.intern("");
    }

    fn first(_args: &[Value]) -> Value {
        if _args.len() > 0 {
            _args[0]
        } else {
            Value::Nil
        }
    }

    #[test]
    fn store_native_function() {
        let mut heap = Heap::new();
        heap.store(Native(first));
    }
}
