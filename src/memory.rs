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
    kind: u8,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Handle {
    ptr: *mut Header,
}

impl Handle {
    pub fn kind(&self) -> u8 {
        unsafe { (*self.ptr).kind }
    }
    fn is_marked(&self) -> bool {
        unsafe { (*self.ptr).is_marked }
    }
    fn mark(&mut self, value: bool) {
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
    const KIND: u8;
    fn trace(&self, collector: &mut Vec<Handle>);
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
    handlers: [Handler; 8],
}

impl Heap {
    pub fn new() -> Self {
        Self {
            handles: Vec::with_capacity(1 << 12),
            handlers: [DefaultHandler; 8],
        }
    }

    pub fn store<T: Traceable>(&mut self, t: T) -> TypedHandle<T> {
        if self.handlers[T::KIND as usize] == DefaultHandler {
            self.handlers[T::KIND as usize] = Handler::of::<T>();
        }
        let typed = TypedHandle::from(t);
        self.handles.push(typed.downgrade());
        typed
    }

    // leave this to the caller
    pub fn collect_garbage(&mut self, roots: Vec<Handle>) {
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
    fn trace(&self, mut roots: Vec<Handle>) {
        while let Some(mut handle) = roots.pop() {
            if handle.is_marked() {
                continue;
            }
            handle.mark(true);
            self.trace_handle(&mut handle, &mut roots);
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
