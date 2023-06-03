use std::{
    alloc::alloc,
    alloc::Layout,
    collections::HashSet,
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

    fn drop_in_place(&self) {
        #[cfg(feature = "log_gc")]
        {
            println!("{:?} free type {:?}", self.ptr, T::KIND);
        }
        unsafe {
            ptr::drop_in_place(self.ptr);
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

pub trait Traceable
where
    Self: Sized,
{
    const KIND: Kind;
    fn cast(handle: &Handle) -> Obj<Self> {
        Obj {
            ptr: handle.ptr as *mut (Header, Self),
        }
    }
    fn obj_from_value(value: Value) -> Option<Obj<Self>> {
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
}

pub struct Heap {
    handles: Vec<Handle>,
    string_pool: HashSet<Obj<String>>,
}

impl Heap {
    pub fn new() -> Self {
        Self {
            handles: Vec::with_capacity(1 << 12),
            string_pool: HashSet::new(),
        }
    }

    pub fn count(&mut self) -> usize {
        self.handles.len()
    }

    pub fn intern(&mut self, name: &str) -> Obj<String> {
        let new_obj = Obj::from(String::from(name));
        match self.string_pool.get(&new_obj) {
            Some(obj) => {
                new_obj.drop_in_place();
                *obj
            }
            None => {
                self.string_pool.insert(new_obj);
                self.handles.push(Handle::from(new_obj));
                new_obj
            }
        }
    }

    pub fn store<T: Traceable>(&mut self, t: T) -> Obj<T> {
        let obj = Obj::from(t);
        self.handles.push(Handle::from(obj));
        #[cfg(feature = "log_gc")]
        {
            println!("{:?} store type {:?}", obj.ptr, T::KIND);
        }
        obj
    }

    pub fn collect_garbage(&mut self, roots: Vec<Handle>) {
        self.mark(roots);
        #[cfg(feature = "log_gc")]
        {
            println!("Starting sweeping.");
        }
        self.sweep();
        #[cfg(feature = "log_gc")]
        {
            println!("Finished tracing references");
        }
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
            handle.accept(&mut roots);
        }

        #[cfg(feature = "log_gc")]
        {
            println!("Done with mark & trace");
        }
    }
    fn sweep(&mut self) {
        // first clean up the string pool
        self.string_pool.retain(|v| v.is_marked());
        let mut index: usize = 0;
        let mut len: usize = self.handles.len();
        loop {
            // look for dead object
            while self.handles[index].is_marked() {
                self.handles[index].mark(false);
                index += 1;
                if index == len {
                    self.handles.truncate(index);
                    return;
                }
            }
            self.handles[index].accept(&mut Demise);
            // look for live object from other end
            loop {
                len -= 1;
                if index == len {
                    self.handles.truncate(len);
                    return;
                }
                if self.handles[len].is_marked() {
                    break;
                }
                self.handles[len].accept(&mut Demise);
            }
            // swap
            self.handles[index] = self.handles[len];
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
        obj.drop_in_place();
    }

    fn visit_class(&mut self, obj: Obj<Class>) -> () {
        obj.drop_in_place();
    }

    fn visit_closure(&mut self, obj: Obj<Closure>) -> () {
        obj.drop_in_place();
    }

    fn visit_function(&mut self, obj: Obj<Function>) -> () {
        obj.drop_in_place();
    }

    fn visit_instance(&mut self, obj: Obj<Instance>) -> () {
        obj.drop_in_place();
    }

    fn visit_native(&mut self, obj: Obj<Native>) -> () {
        obj.drop_in_place();
    }

    fn visit_string(&mut self, obj: Obj<String>) -> () {
        obj.drop_in_place();
    }

    fn visit_upvalue(&mut self, obj: Obj<Upvalue>) -> () {
        obj.drop_in_place();
    }
}

type Collector = Vec<Handle>;

impl ObjVisitor<()> for Collector {
    fn visit_bound_method(&mut self, obj: Obj<BoundMethod>) {
        self.push(Handle::from(obj.receiver));
        self.push(Handle::from(obj.method));
    }

    fn visit_class(&mut self, obj: Obj<Class>) {
        self.push(Handle::from(obj.name));
        for (name, method) in &obj.methods {
            self.push(Handle::from(*name));
            self.push(Handle::from(*method));
        }
    }

    fn visit_closure(&mut self, obj: Obj<Closure>) {
        self.push(Handle::from(obj.function));
        for upvalue in obj.upvalues.iter() {
            self.push(Handle::from(*upvalue));
        }
    }

    fn visit_function(&mut self, obj: Obj<Function>) {
        if let Some(name) = &obj.name {
            self.push(Handle::from(*name))
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
            Upvalue::Open(_, Some(next)) => self.push(Handle::from(next)),
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
