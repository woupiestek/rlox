// run time data structures

use std::{
    alloc::Layout,
    alloc::{alloc, dealloc},
    collections::HashMap,
    ptr,
};

use crate::class::{Class, Method, Path, Symbol};

pub struct Obj {
    pub is_marked: bool,
    kind: Kind,
}

pub struct TypedHandle<T> {
    obj: *mut (Obj, T),
}
// generics seem to be the issue here
impl<T> Copy for TypedHandle<T> {}
impl<T> Clone for TypedHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> TypedHandle<T> {
    fn build(kind: Kind, body: T) -> Self {
        unsafe {
            let obj = alloc(kind.layout()) as *mut (Obj, T);
            (*obj).0.is_marked = false;
            (*obj).0.kind = kind;
            (*obj).1 = body; // is this cleaned up?
            Self { obj }
        }
    }

    fn demolish(self) {
        unsafe {
            // does this solve our problem?
            ptr::drop_in_place(&mut (*self.obj).1);
            dealloc(self.obj as *mut u8, (*self.obj).0.kind.layout());
        }
    }

    fn forget_type(self) -> Handle {
        unsafe {
            Handle {
                obj: self.obj as *mut Obj,
            }
        }
    }

    pub fn constructor(class: Path<Class>) -> TypedHandle<Constructor> {
        TypedHandle::build(
            Kind::Constructor,
            Constructor {
                class,
                upvalues: Vec::new(),
            },
        )
    }

    pub fn instance(constructor: TypedHandle<Constructor>) -> TypedHandle<Instance> {
        TypedHandle::build(
            Kind::Instance,
            Instance {
                constructor,
                fields: HashMap::new(),
            },
        )
    }

    pub fn bound_method(
        receiver: TypedHandle<Instance>,
        method: Path<Method>,
    ) -> TypedHandle<BoundMethod> {
        TypedHandle::build(Kind::BoundMethod, BoundMethod { receiver, method })
    }

    pub fn string(body: String) -> TypedHandle<String> {
        TypedHandle::build(Kind::String, body)
    }

    pub fn upvalue(location: *mut Value) -> TypedHandle<Upvalue> {
        TypedHandle::build(
            Kind::Upvalue,
            Upvalue {
                location,
                closed: None,
            },
        )
    }
}

#[derive(Copy, Clone)]
pub struct Handle {
    pub obj: *mut Obj,
}

impl Handle {
    pub unsafe fn kind(&self) -> &Kind {
        &((*(self.obj)).kind)
    }

    pub unsafe fn cast<U>(&self) -> TypedHandle<U> {
        TypedHandle {
            obj: self.obj as *mut (Obj, U),
        }
    }

    pub unsafe fn body<U>(&self) -> &mut U {
        &mut ((*(self.obj as *mut (Obj, U))).1)
    }

    pub fn mark(&mut self, gray: &mut Vec<Handle>) {
        unsafe {
            if (*self.obj).is_marked {
                return;
            }
            (*self.obj).is_marked = true;
            gray.push(*self);
        }
    }

    pub fn destroy(&mut self) {
        unsafe {
            match self.kind() {
                Kind::Constructor => {
                    self.cast::<Constructor>().demolish();
                }
                Kind::Instance => {
                    self.cast::<Instance>().demolish();
                }
                Kind::BoundMethod => {
                    self.cast::<BoundMethod>().demolish();
                }
                Kind::String => {
                    self.cast::<String>().demolish();
                }
                Kind::Upvalue => {
                    self.cast::<Upvalue>().demolish();
                }
            }
        }
    }

    pub unsafe fn trace(&mut self, gray: &mut Vec<Handle>) {
        match self.kind() {
            Kind::Constructor => {
                let cons: &mut Constructor = self.body();
                for upvalue in cons.upvalues.iter_mut() {
                    upvalue.forget_type().mark(gray);
                }
            }
            Kind::Instance => {
                let ins: &mut Instance = self.body();
                for value in ins.fields.values_mut() {
                    value.mark(gray);
                }
                ins.constructor.forget_type().mark(gray);
            }
            Kind::BoundMethod => {
                let bm: &mut BoundMethod = self.body();
                bm.receiver.forget_type().mark(gray);
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

#[derive(Copy, Clone)]
pub enum Value {
    Nil,
    True,
    False,
    Number(f64),
    Obj(Handle),
    Native(fn(args: &[Value]) -> Value),
}

impl Value {
    pub fn mark(&mut self, gray: &mut Vec<Handle>) {
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
            Kind::Constructor => Layout::new::<(Obj, Constructor)>(),
            Kind::Instance => Layout::new::<(Obj, Instance)>(),
            Kind::BoundMethod => Layout::new::<(Obj, BoundMethod)>(),
            Kind::String => Layout::new::<(Obj, String)>(),
            Kind::Upvalue => Layout::new::<(Obj, Upvalue)>(),
        }
    }
}

pub struct Upvalue {
    location: *mut Value,
    closed: Option<Value>,
}

// I guess the constructor can own the upvalues,
// though the class basically already determines how many are needed.
pub struct Constructor {
    class: Path<Class>,                  // maybe trouble
    upvalues: Vec<TypedHandle<Upvalue>>, //trouble
}

pub struct Instance {
    constructor: TypedHandle<Constructor>,
    fields: HashMap<Symbol, Value>, // trouble
}

pub struct BoundMethod {
    receiver: TypedHandle<Instance>,
    method: Path<Method>, // maybe trouble
}

pub enum RuntimeError {
    ArityMismatch,
    StackOverflow,
    OutOfMemory,
    FieldNotFound,
}
