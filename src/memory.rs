use std::{
    fmt::Display,
    ops::{Deref, DerefMut},
};

use crate::{
    loxtr::Loxtr,
    object::{BoundMethod, Class, Closure, Function, Instance, Native, Upvalue, Value},
    table::Table,
};

// note that usually 8 byte is allocated for this due to alignment, so plenty of space!

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Kind {
    BoundMethod = 1,
    Class,
    Closure,
    Function,
    Instance,
    Native,
    String,
    Upvalue,
}

type Obj<T> = (Kind, bool, T);

macro_rules! as_traceable {
    ($handle:expr, $method:ident($($args:tt)*)) => {
        match $handle.kind() {
            Kind::BoundMethod => BoundMethod::cast(&$handle).$method($($args)*),
            Kind::Class => Class::cast(&$handle).$method($($args)*),
            Kind::Closure => Closure::cast(&$handle).$method($($args)*),
            Kind::Function => Function::cast(&$handle).$method($($args)*),
            Kind::Instance => Instance::cast(&$handle).$method($($args)*),
            Kind::Native => Native::cast(&$handle).$method($($args)*),
            Kind::String => Loxtr::cast(&$handle).$method($($args)*),
            Kind::Upvalue => Upvalue::cast(&$handle).$method($($args)*),
        }
    };
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Handle {
    // Obj<()> did not work! perhaps it is an zero size type issue
    ptr: *mut Obj<u8>,
}

impl Handle {
    pub fn kind(&self) -> Kind {
        unsafe { (*self.ptr).0 }
    }
    fn is_marked(&self) -> bool {
        unsafe { (*self.ptr).1 }
    }
    fn mark(&mut self, value: bool) {
        #[cfg(feature = "log_gc")]
        {
            println!("{:?} mark {} as {}", self.ptr, self, value);
        }
        unsafe { (*self.ptr).1 = value }
    }
    fn free(&self) -> usize {
        as_traceable!(self, free())
    }
}

impl Display for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        as_traceable!(self, fmt(f))
    }
}

impl<T: Traceable> From<GC<T>> for Handle {
    fn from(value: GC<T>) -> Self {
        Self {
            ptr: value.ptr as *mut Obj<u8>,
        }
    }
}

pub struct GC<Body: Traceable> {
    ptr: *mut Obj<Body>,
}

impl<T: Traceable> GC<T> {
    pub fn is_marked(&self) -> bool {
        unsafe { (*self.ptr).1 }
    }

    fn free(&self) -> usize {
        #[cfg(feature = "log_gc")]
        {
            println!("{:?} free as {:?}", self.ptr, T::KIND);
        }
        unsafe {
            let count = self.byte_count();
            drop(Box::from_raw(self.ptr));
            count
        }
    }
}

impl<T: Traceable> Copy for GC<T> {}

impl<T: Traceable> Clone for GC<T> {
    fn clone(&self) -> Self {
        Self { ptr: self.ptr }
    }
}

impl<T: Traceable> std::fmt::Debug for GC<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GC").field("ptr", &self.ptr).finish()
    }
}

impl<T: Traceable> Deref for GC<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &(*self.ptr).2 }
    }
}

impl<T: Traceable> DerefMut for GC<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut (*self.ptr).2 }
    }
}

impl<T: Traceable> PartialEq for GC<T> {
    fn eq(&self, other: &Self) -> bool {
        self.ptr == other.ptr
    }
}

pub trait Traceable
where
    Self: Sized + Display,
{
    const KIND: Kind;
    fn byte_count(&self) -> usize;
    fn cast(handle: &Handle) -> GC<Self> {
        GC {
            ptr: handle.ptr as *mut Obj<Self>,
        }
    }
    fn nullable(value: Value) -> Option<GC<Self>> {
        if let Value::Object(handle) = value {
            if handle.kind() == Self::KIND {
                Some(Self::cast(&handle))
            } else {
                None
            }
        } else {
            None
        }
    }
    fn trace(&self, collector: &mut Vec<Handle>);
}

impl<T: Traceable> From<Value> for GC<T> {
    fn from(value: Value) -> Self {
        if let Value::Object(handle) = value {
            assert_eq!(handle.kind(), T::KIND);
            T::cast(&handle)
        } else {
            panic!("cannot cast {} to {:?}", value, T::KIND)
        }
    }
}

pub struct Heap {
    handles: Vec<Handle>,
    string_pool: Table<()>,
    byte_count: usize,
}

impl Heap {
    pub fn new() -> Self {
        Self {
            handles: Vec::with_capacity(1 << 12),
            string_pool: Table::new(),
            byte_count: 0,
        }
    }

    pub fn increase_byte_count(&mut self, diff: usize) {
        self.byte_count += diff;
    }

    pub fn byte_count(&mut self) -> usize {
        self.byte_count
    }

    pub fn intern(&mut self, name: &str) -> GC<Loxtr> {
        if let Some(gc) = self.string_pool.find_key(name) {
            gc
        } else {
            let gc = self.store(Loxtr::copy(name));
            self.string_pool.set(gc, ());
            gc
        }
    }

    pub fn store<T: Traceable>(&mut self, t: T) -> GC<T> {
        let obj = GC {
            ptr: Box::into_raw(Box::from((T::KIND, false, t))),
        };
        self.handles.push(Handle::from(obj));
        self.byte_count += obj.byte_count();
        #[cfg(feature = "log_gc")]
        {
            let kind: Kind = unsafe { (*obj.ptr).0 };
            println!("{:?} store as {:?}", obj.ptr, kind);
        }
        obj
    }

    pub fn collect_garbage(&mut self, roots: Vec<Handle>) {
        self.mark(roots);
        self.sweep();
    }

    fn mark(&self, mut roots: Vec<Handle>) {
        #[cfg(feature = "log_gc")]
        {
            println!(
                "Start marking objects & tracing references. Number of roots: {}",
                roots.len()
            );
        }

        while let Some(mut handle) = roots.pop() {
            if handle.is_marked() {
                continue;
            }
            handle.mark(true);
            as_traceable!(handle, trace(&mut roots));
        }

        #[cfg(feature = "log_gc")]
        {
            println!("Done with mark & trace");
        }
    }
    fn sweep(&mut self) {
        #[cfg(feature = "log_gc")]
        {
            println!("Start sweeping.");
        }
        // first clean up the string pool
        self.string_pool.sweep();
        let mut index: usize = 0;
        let mut len: usize = self.handles.len();
        'a: loop {
            // look for dead object
            while self.handles[index].is_marked() {
                self.handles[index].mark(false);
                index += 1;
                if index == len {
                    break 'a;
                }
            }
            self.byte_count -= self.handles[index].free();
            // look for live object from other end
            loop {
                len -= 1;
                if index == len {
                    break 'a;
                }
                if self.handles[len].is_marked() {
                    break;
                }
                self.byte_count -= self.handles[len].free();
            }
            // swap
            self.handles[index] = self.handles[len];
        }
        self.handles.truncate(len);
        #[cfg(feature = "log_gc")]
        {
            println!("Done sweeping");
        }
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        while let Some(handle) = self.handles.pop() {
            handle.free();
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

    fn first(_args: &[Value]) -> Result<Value, String> {
        if _args.len() > 0 {
            Ok(_args[0])
        } else {
            err!("Too few arguments.")
        }
    }

    #[test]
    fn store_native_function() {
        let mut heap = Heap::new();
        heap.store(Native(first));
    }
}
