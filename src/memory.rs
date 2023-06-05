use std::{
    alloc::alloc,
    alloc::Layout,
    collections::HashSet,
    fmt::Display,
    ops::{Deref, DerefMut},
    ptr,
};

use crate::{
    loxtr::Loxtr,
    object::{BoundMethod, Class, Closure, Function, Instance, Native, Upvalue, Value},
};

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
    is_marked: bool,
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
        #[cfg(feature = "log_gc")]
        {
            println!("{:?} mark {} as {}", self.ptr, self, value);
        }
        unsafe { (*self.ptr).is_marked = value }
    }
}

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

impl Display for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        as_traceable!(self, fmt(f))
    }
}

impl<T: Traceable> From<Obj<T>> for Handle {
    fn from(value: Obj<T>) -> Self {
        Self {
            ptr: value.ptr as *mut Header,
        }
    }
}

pub struct Obj<Body: Traceable> {
    ptr: *mut (Header, Body),
}

impl<T: Traceable> Obj<T> {
    fn is_marked(&self) -> bool {
        unsafe { (*self.ptr).0.is_marked }
    }

    fn free(&self) -> usize {
        #[cfg(feature = "log_gc")]
        {
            println!("{:?} free {}", self.ptr, **self);
        }
        unsafe {
            let count = self.byte_count();
            ptr::drop_in_place(self.ptr);
            count
        }
    }
}

impl<T: Traceable> Copy for Obj<T> {}

impl<T: Traceable> Clone for Obj<T> {
    fn clone(&self) -> Self {
        *self
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

impl<T: Traceable> From<T> for Obj<T> {
    fn from(t: T) -> Self {
        unsafe {
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
        }
    }
}

pub trait Tracer {
    type Target;
}

pub trait Traceable
where
    Self: Sized + Display,
{
    const KIND: Kind;
    fn byte_count(&self) -> usize;
    fn cast(handle: &Handle) -> Obj<Self> {
        Obj {
            ptr: handle.ptr as *mut (Header, Self),
        }
    }
    fn nullable(value: Value) -> Option<Obj<Self>> {
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

impl<T: Traceable> From<Value> for Obj<T> {
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
    string_pool: HashSet<Obj<Loxtr>>,
    byte_count: usize,
}

impl Heap {
    pub fn new() -> Self {
        Self {
            handles: Vec::with_capacity(1 << 12),
            string_pool: HashSet::new(),
            byte_count: 0,
        }
    }

    pub fn increase_byte_count(&mut self, diff: usize) {
        self.byte_count += diff;
    }

    pub fn byte_count(&mut self) -> usize {
        self.byte_count
    }

    pub fn intern(&mut self, name: &str) -> Obj<Loxtr> {
        let new_str = Obj::from(Loxtr::copy(name));
        match self.string_pool.get(&new_str) {
            Some(obj) => {
                new_str.free();
                *obj
            }
            None => {
                self.string_pool.insert(new_str);
                self.handles.push(Handle::from(new_str));
                self.byte_count += new_str.byte_count();
                new_str
            }
        }
    }

    pub fn store<T: Traceable>(&mut self, t: T) -> Obj<T> {
        let obj = Obj::from(t);
        self.handles.push(Handle::from(obj));
        self.byte_count += obj.byte_count();
        #[cfg(feature = "log_gc")]
        {
            println!("{:?} store type {:?}", obj.ptr, T::KIND);
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
        self.string_pool.retain(|v| v.is_marked());
        let mut index: usize = 0;
        let mut len: usize = self.handles.len();
        let mut byte_count: usize = 0;
        'a: loop {
            // look for dead object
            while self.handles[index].is_marked() {
                self.handles[index].mark(false);
                byte_count += as_traceable!(self.handles[index], byte_count());
                index += 1;
                if index == len {
                    break 'a;
                }
            }
            self.byte_count -= as_traceable!(self.handles[index], free());
            // look for live object from other end
            loop {
                len -= 1;
                if index == len {
                    break 'a;
                }
                if self.handles[len].is_marked() {
                    break;
                }
                self.byte_count -= as_traceable!(self.handles[len], free());
            }
            // swap
            self.handles[index] = self.handles[len];
        }
        assert_eq!(byte_count, self.byte_count);
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
            // no point to adjusting byte counts now, but some appear to be unaccounted for!
            as_traceable!(handle, free());
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
