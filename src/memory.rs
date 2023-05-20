use std::{
    alloc::alloc,
    alloc::Layout,
    ops::{Deref, DerefMut},
    ptr,
};

// note that usually 8 byte is allocated for this due to alignment, so plenty of space!

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
    pub fn is_marked(&self) -> bool {
        unsafe { (*self.ptr).is_marked }
    }
    pub fn mark(&mut self, value: bool) {
        unsafe { (*self.ptr).is_marked = value }
    }
    pub fn upgrade<T: Traceable>(&self) -> Option<TypedHandle<T>> {
        if T::KIND == self.kind() {
            Some(TypedHandle {
                ptr: self.ptr as *mut (Header, T),
            })
        } else {
            None
        }
    }
}

pub struct TypedHandle<Body> {
    ptr: *mut (Header, Body),
}

impl<T: Traceable> TypedHandle<T> {
    // heap argument? method on heap?
    fn from(t: T) -> Self {
        unsafe {
            let ptr = alloc(Layout::new::<(Header, T)>()) as *mut (Header, T);
            assert!(!ptr.is_null());
            (*ptr).0.is_marked = false;
            (*ptr).0.kind = T::KIND;
            (*ptr).1 = t;
            TypedHandle { ptr }
        }
    }
    pub fn downgrade(&self) -> Handle {
        Handle {
            ptr: self.ptr as *mut Header,
        }
    }
    fn drop(self) {
        unsafe { ptr::drop_in_place(self.ptr) }
    }
}

impl<T> Deref for TypedHandle<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &(*self.ptr).1 }
    }
}

impl<T> DerefMut for TypedHandle<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut (*self.ptr).1 }
    }
}

pub trait Traceable
where
    Self: Sized,
{
    const KIND: Kind;
    // perhaps have a heap argument here, as with the collector.

    fn trace(&self, collector: &mut Vec<Handle>);
    fn is_type_of(handle: &Handle) -> bool {
        handle.kind() == Self::KIND
    }
    fn upgrade(handle: &Handle) -> Option<TypedHandle<Self>> {
        if Self::KIND == handle.kind() {
            Some(TypedHandle {
                ptr: handle.ptr as *mut (Header, Self),
            })
        } else {
            None
        }
    }
}

struct Handler {
    drop: fn(Handle),
    trace: fn(&mut Handle, &mut Vec<Handle>),
}

impl PartialEq for Handler {
    fn eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl Handler {
    pub fn of<T: Traceable>() -> Self {
        Self {
            drop: |handle| {
                T::upgrade(&handle).unwrap().drop();
            },
            trace: |handle, collector| {
                T::upgrade(handle).unwrap().trace(collector);
            },
        }
    }
}
const DefaultHandler: Handler = Handler {
    drop: |_handle| panic!(),
    trace: |_handle, _collector| panic!(),
};

pub struct Heap {
    handles: Vec<Handle>,
    // my answer to the big match statement...
    handlers: [Handler; 5],
}

impl Heap {
    pub fn new() -> Self {
        Self {
            handles: Vec::with_capacity(1 << 12),
            handlers: [DefaultHandler; 5],
        }
    }
    // todo: this has to in fact keep track of allocated bytes,
    // then trigger garbage collection,
    // which requires knowlegde of all fucking roots.
    // creating a new object depends on everything!
    pub fn store<T: Traceable>(&mut self, t: T) -> TypedHandle<T> {
        if self.handlers[T::KIND as usize] == DefaultHandler {
            self.handlers[T::KIND as usize] = Handler::of::<T>();
        }
        let typed = TypedHandle::from(t);
        self.handles.push(typed.downgrade());
        typed
    }

    // leave this to the caller
    pub fn collect_garbage(&mut self, mut roots: Vec<Handle>) {
        self.trace(roots);
        self.sweep();
    }

    pub fn count_objects(&self) -> usize {
        self.handles.len()
    }

    fn get_handler(&self, handle: &Handle) -> &Handler {
        &self.handlers[handle.kind() as usize]
    }
    fn drop_handle(&self, handle: Handle) {
        (self.get_handler(&handle).drop)(handle)
    }
    fn trace_handle(&self, handle: &mut Handle, collector: &mut Vec<Handle>) {
        (self.get_handler(&handle).trace)(handle, collector)
    }
    // posess roots.
    fn trace(&self, mut roots: Vec<Handle>) {
        while let Some(mut handle) = roots.pop() {
            handle.mark(true);
            self.trace_handle(&mut handle, &mut roots);
        }
    }
    fn sweep(&mut self) {
        let mut index: usize = 0;
        while index + 1 < self.handles.len() {
            // look for live object from the end
            let live = self.handles.pop().unwrap();
            if !live.is_marked() {
                self.drop_handle(live);
                continue;
            }
            // look for a dead object from the bottom
            loop {
                let mut dead = self.handles[index];
                if dead.is_marked() {
                    dead.mark(false);
                    index += 1
                } else {
                    // swap
                    self.drop_handle(dead);
                    self.handles[index] = live;
                    // repeat
                    break;
                }
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

// this now remains as the only link to the concrete types,
// but it does not have to be this way.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Kind {
    Constructor,
    Instance,
    BoundMethod,
    String,
    Upvalue,
}
