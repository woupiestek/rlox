use std::{
    alloc::alloc,
    alloc::Layout,
    collections::HashMap,
    fmt::Display,
    ops::{Deref, DerefMut},
    ptr,
};

use crate::object::{
    BoundMethod, Class, Closure, Function, Instance, Native, ObjVisitor, Upvalue, Value,
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
        #[cfg(feature = "log_gc")]
        {
            println!("{:?} mark {}", self.ptr, self);
        }
        unsafe { (*self.ptr).is_marked = value }
    }
    pub fn accept<T>(&self, obj_visitor: &mut dyn ObjVisitor<T>) -> T {
        match self.kind() {
            Kind::BoundMethod => obj_visitor.visit_bound_method(BoundMethod::cast(self)),
            Kind::Class => obj_visitor.visit_class(Class::cast(self)),
            Kind::Closure => obj_visitor.visit_closure(Closure::cast(self)),
            Kind::Function => obj_visitor.visit_function(Function::cast(self)),
            Kind::Instance => obj_visitor.visit_instance(Instance::cast(self)),
            Kind::Native => obj_visitor.visit_native(Native::cast(self)),
            Kind::String => obj_visitor.visit_string(String::cast(self)),
            Kind::Upvalue => obj_visitor.visit_upvalue(Upvalue::cast(self)),
        }
    }
}

impl ObjVisitor<std::fmt::Result> for std::fmt::Formatter<'_> {
    fn visit_bound_method(&mut self, obj: Obj<BoundMethod>) -> std::fmt::Result {
        write!(self, "{}", *obj.method.function)
    }

    fn visit_class(&mut self, obj: Obj<Class>) -> std::fmt::Result {
        write!(self, "<class {}>", *obj.name)
    }

    fn visit_closure(&mut self, obj: Obj<Closure>) -> std::fmt::Result {
        write!(self, "{}", *obj.function)
    }

    fn visit_function(&mut self, obj: Obj<Function>) -> std::fmt::Result {
        write!(self, "{}", *obj)
    }

    fn visit_instance(&mut self, obj: Obj<Instance>) -> std::fmt::Result {
        write!(self, "{} instance", *obj.class.name)
    }

    fn visit_native(&mut self, _obj: Obj<Native>) -> std::fmt::Result {
        write!(self, "{}", "<native fn>")
    }

    fn visit_string(&mut self, obj: Obj<String>) -> std::fmt::Result {
        write!(self, "{}", *obj)
    }

    fn visit_upvalue(&mut self, _obj: Obj<Upvalue>) -> std::fmt::Result {
        write!(self, "{}", "<upvalue>")
    }
}

impl Display for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.accept(f)
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
    fn test_handle(handle: &Handle) -> bool {
        handle.kind() == Self::KIND
    }
    fn cast(handle: &Handle) -> Obj<Self> {
        Obj {
            ptr: handle.ptr as *mut (Header, Self),
        }
    }
    fn obj_from_handle(handle: &Handle) -> Option<Obj<Self>> {
        if Self::test_handle(handle) {
            Some(Self::cast(handle))
        } else {
            None
        }
    }

    fn obj_from_value(value: Value) -> Option<Obj<Self>> {
        if let Value::Object(handle) = value {
            Self::obj_from_handle(&handle)
        } else {
            None
        }
    }
}
macro_rules! drop_handle {
    ($handle:expr,$traceable:ty) => {{
        #[cfg(feature = "log_gc")]
        {
            println!("{:?} free type {:?}", $handle.ptr, <$traceable>::KIND);
        }
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

    pub fn collect_garbage(&mut self, roots: Vec<Handle>) {
        self.trace(roots);
        self.sweep();
    }

    fn trace(&self, mut roots: Vec<Handle>) {
        while let Some(mut handle) = roots.pop() {
            if handle.is_marked() {
                continue;
            }
            handle.mark(true);
            handle.accept(&mut roots);
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
                    dead.accept(&mut Demise);
                    self.handles[index] = live;
                    live.mark(false);
                    index += 1;
                    break;
                }
                live.accept(&mut Demise)
            }
        }
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        while let Some(handle) = self.handles.pop() {
            handle.accept(&mut Demise)
        }
    }
}

struct Demise;

impl ObjVisitor<()> for Demise {
    fn visit_bound_method(&mut self, obj: Obj<BoundMethod>) -> () {
        drop_handle!(obj, BoundMethod)
    }

    fn visit_class(&mut self, obj: Obj<Class>) -> () {
        drop_handle!(obj, Class)
    }

    fn visit_closure(&mut self, obj: Obj<Closure>) -> () {
        drop_handle!(obj, Closure)
    }

    fn visit_function(&mut self, obj: Obj<Function>) -> () {
        drop_handle!(obj, Function)
    }

    fn visit_instance(&mut self, obj: Obj<Instance>) -> () {
        drop_handle!(obj, Instance)
    }

    fn visit_native(&mut self, obj: Obj<Native>) -> () {
        drop_handle!(obj, Native)
    }

    fn visit_string(&mut self, obj: Obj<String>) -> () {
        drop_handle!(obj, String)
    }

    fn visit_upvalue(&mut self, obj: Obj<Upvalue>) -> () {
        drop_handle!(obj, Upvalue)
    }
}

type Collector = Vec<Handle>;

impl ObjVisitor<()> for Collector {
    fn visit_bound_method(&mut self, obj: Obj<BoundMethod>) {
        self.push(obj.receiver.as_handle());
        self.push(obj.method.as_handle());
    }

    fn visit_class(&mut self, obj: Obj<Class>) {
        self.push(obj.name.as_handle());
        for (name, method) in &obj.methods {
            self.push(name.as_handle());
            self.push(method.as_handle());
        }
    }

    fn visit_closure(&mut self, obj: Obj<Closure>) {
        self.push(obj.function.as_handle());
        for upvalue in obj.upvalues.iter() {
            self.push(upvalue.as_handle());
        }
    }

    fn visit_function(&mut self, obj: Obj<Function>) {
        if let Some(n) = &obj.name {
            self.push(n.as_handle())
        }
        for value in &obj.chunk.constants {
            if let Value::Object(h) = value {
                self.push(*h)
            }
        }
    }

    fn visit_instance(&mut self, obj: Obj<Instance>) {
        for value in obj.properties.values() {
            if let Value::Object(handle) = value {
                self.push(*handle)
            }
        }
    }

    fn visit_native(&mut self, _obj: Obj<Native>) {}

    fn visit_string(&mut self, _obj: Obj<String>) {}

    fn visit_upvalue(&mut self, obj: Obj<Upvalue>) {
        match *obj {
            Upvalue::Open(_, Some(next)) => self.push(next.as_handle()),
            Upvalue::Closed(Value::Object(handle)) => self.push(handle),
            _ => (),
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
