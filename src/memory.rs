use std::{
    alloc::alloc,
    alloc::Layout,
    mem,
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

pub struct Obj<Body: Traceable> {
    ptr: *mut (Header, Body),
}

impl<T: Traceable> Obj<T> {
    pub fn downgrade(&self) -> Handle {
        Handle {
            ptr: self.ptr as *mut Header,
        }
    }
    pub fn as_value(&self) -> Value {
        Value::Object(self.downgrade())
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
    fn upgrade(handle: &Handle) -> Result<Obj<Self>, String> {
        if Self::KIND == handle.kind() {
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
    fn cast(handle: &Handle) -> Result<&Self, String> {
        if Self::KIND == handle.kind() {
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
}

impl Heap {
    pub fn new() -> Self {
        Self {
            handles: Vec::with_capacity(1 << 12),
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
        self.handles.push(typed.downgrade());
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
        heap.store("".to_string());
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
