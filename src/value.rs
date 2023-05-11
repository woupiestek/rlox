// chunk, object, value: the runtime data model

use std::{collections::HashMap, fmt::Debug};

// so I can change my mind later
type Pointer<T> = *mut T;

// aren't these supposed to be the same size?
// moved string & nativefn: those seem distinct enough
#[derive(Debug,PartialEq)]
pub enum Value {
    Bool(bool),
    Nil,
    Number(f64),
    String(Pointer< LoxString>),
    Object(Pointer<Obj>),
    Native(Pointer<Native>),
}

#[derive(Debug)]
pub struct Obj {
    isMarked: bool,
    next: Option<Pointer<Obj>>,
    body: ObjBody,
}

pub type NativeFn = fn(arg_count: usize, args: &[Value]) -> Value;

pub struct Native {
    function: NativeFn
}
impl Debug for  Native {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "<native fn>")
    }
}

#[derive(Debug)]
pub struct Class {
    pub name: LoxString,
    methods: HashMap<LoxString, Closure>,
}

impl Class {
    pub fn new(name: LoxString) -> Self {
        Self {
            name,
            methods: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct Closure {
    function: LoxFunction,
    up_values: Vec<UpValue>,
}

impl Closure {
    pub fn new(function: LoxFunction) -> Self {
        Self {
            function,
            up_values: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct LoxString {
    pub chars: Vec<u8>,
    pub hash: u32,
}

impl LoxString {
    pub fn hash(chars: &Vec<u8>) -> u32 {
        let mut hash = 2166136261u32;
        for i in 0..chars.len() {
            hash ^= chars[i] as u32;
            hash *= 16777619;
        }
        return hash;
    }
    pub fn take(chars: Vec<u8>) -> LoxString {
        let hash = LoxString::hash(&chars);
        // string should be interned!
        LoxString { chars, hash }
    }
    // let's see if this lasts
    pub fn copy(const_chars: &[u8]) -> LoxString {
        LoxString::take(const_chars.to_vec())
    }
}

#[derive(Debug)]
pub struct LoxFunction {
    name: Option<LoxString>,
    arity: u16,
    up_value_count: u16,
    chunk: Chunk,
}

impl LoxFunction {
    pub fn new() -> Self {
        Self {
            name: None,
            arity: 0,
            up_value_count: 0,
            chunk: Chunk::new(),
        }
    }
}

#[derive(Debug)]
pub struct UpValue {
    location: Pointer<Value>,
    closed: Value,
    next: Option<Pointer<UpValue>>,
}

impl UpValue {
    pub fn new (location:Pointer<Value>)->UpValue {
        UpValue { location, closed: Value::Nil, next: None }
    }
}

#[derive(Debug)]
pub enum ObjBody {
    BoundMethod {
        receiver: Value,
        method: Closure,
    },
    Class(Class),
    Closure(Closure),
    Function(LoxFunction),
    Instance {
        class: Class,
        fields: HashMap<LoxString, Value>,
    },
    UpValue(UpValue),
}

#[derive(Debug)]
struct Chunk {
    code: Vec<u8>,
    lines: Vec<u16>,
    constants: Vec<Value>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            lines: Vec::new(),
            constants: Vec::new(),
        }
    }
    pub fn write(&mut self, op_code: u8, line: u16) {
        self.code.push(op_code);
        self.lines.push(line);
    }
    pub fn add_constant(&mut self, constant: Value) -> usize {
        self.constants.push(constant);
        self.constants.len() - 1
    }
}
