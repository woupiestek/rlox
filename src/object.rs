// run time data structures

use std::{
    alloc::Layout,
    alloc::{alloc, dealloc},
    collections::HashMap,
};

use crate::class::{Class, Method, Path, Symbol};

pub struct Obj<T> {
    pub is_marked: bool,
    kind: Kind,
    body: T,
}

pub struct Handle<T> {
    pub obj: *mut Obj<T>,
}

impl<T> Handle<T> {
    fn build(kind: Kind, body: T) -> Handle<T> {
        unsafe {
            let obj = alloc(kind.layout()) as *mut Obj<T>;
            (*obj).is_marked = false;
            (*obj).kind = kind;
            (*obj).body = body;
            Handle { obj }
        }
    }
    fn cast<U>(&self) -> Handle<U> {
        Handle {
            obj: self.obj as *mut Obj<U>,
        }
    }
    pub fn down_cast(&self) -> Handle<()> {
        self.cast::<()>()
    }

    pub fn mark(&mut self, gray: &mut Vec<Handle<()>>) {
        unsafe {
            if (*self.obj).is_marked {
                return;
            }
            (*self.obj).is_marked = true;
            gray.push(self.down_cast());
        }
    }
}

impl Handle<()> {
    pub unsafe fn body<U>(&self) -> &mut U {
        &mut ((*(self.cast::<U>().obj)).body)
    }

    pub fn constructor(class: Path<Class>) -> Handle<Constructor> {
        Handle::build(
            Kind::Constructor,
            Constructor {
                class,
                upvalues: Vec::new(),
            },
        )
    }

    pub fn instance(constructor: Handle<Constructor>) -> Handle<Instance> {
        Handle::build(
            Kind::Instance,
            Instance {
                constructor,
                fields: HashMap::new(),
            },
        )
    }

    pub fn bound_method(receiver: Handle<Instance>, method: Path<Method>) -> Handle<BoundMethod> {
        Handle::build(Kind::BoundMethod, BoundMethod { receiver, method })
    }

    pub fn string(body: String) -> Handle<String> {
        Handle::build(Kind::String, body)
    }

    pub fn upvalue(location: *mut Value) -> Handle<Upvalue> {
        Handle::build(
            Kind::Upvalue,
            Upvalue {
                location,
                closed: None,
            },
        )
    }

    pub fn destroy(&mut self) {
        unsafe {
            dealloc(self.obj as *mut u8, (*self.obj).kind.layout());
        }
    }

    pub unsafe fn trace(&mut self, gray: &mut Vec<Handle<()>>) {
        match (*self.obj).kind {
            Kind::Constructor => {
                let cons: &mut Constructor = self.body();
                for upvalue in cons.upvalues.iter_mut() {
                    upvalue.down_cast().mark(gray);
                }
            }
            Kind::Instance => {
                let ins: &mut Instance = self.body();
                for value in ins.fields.values_mut() {
                    value.mark(gray);
                }
                ins.constructor.mark(gray);
            }
            Kind::BoundMethod => {
                let bm: &mut BoundMethod = self.body();
                bm.receiver.mark(gray);
            }
            Kind::String => (),
            Kind::Upvalue => {
                let upvalue: &mut Upvalue = self.body();
                if let Some(mut value) = upvalue.closed {
                    value.mark(gray);
                }
            }
        }
    }
}

// just like that !?
impl<T> Copy for Handle<T> {}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Self { obj: self.obj }
    }
}

#[derive(Copy, Clone)]
pub enum Value {
    Nil,
    True,
    False,
    Number(f64),
    Obj(Handle<Obj<()>>),
    Native(fn(args: &[Value]) -> Value),
}

impl Value {
    pub fn mark(&mut self, gray: &mut Vec<Handle<()>>) {
        if let Value::Obj(mut handle) = self {
            handle.mark(gray)
        }
    }
}

pub enum Kind {
    Constructor,
    Instance,
    BoundMethod,
    String,
    Upvalue,
}

impl Kind {
    pub fn layout(&self) -> Layout {
        match self {
            Kind::Constructor => Layout::new::<Obj<Constructor>>(),
            Kind::Instance => Layout::new::<Obj<Instance>>(),
            Kind::BoundMethod => Layout::new::<Obj<BoundMethod>>(),
            Kind::String => Layout::new::<Obj<String>>(),
            Kind::Upvalue => Layout::new::<Obj<Upvalue>>(),
        }
    }
}

pub struct Upvalue {
    location: *mut Value,
    closed: Option<Value>,
}

pub struct Constructor {
    class: Path<Class>,
    upvalues: Vec<Handle<Upvalue>>,
}

pub struct Instance {
    constructor: Handle<Constructor>,
    fields: HashMap<Symbol, Value>,
}

pub struct BoundMethod {
    receiver: Handle<Instance>,
    method: Path<Method>,
}

pub enum RuntimeError {
    ArityMismatch,
    StackOverflow,
    OutOfMemory,
    FieldNotFound,
}
