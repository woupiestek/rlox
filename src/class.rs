// byte code data structures

pub struct Path<T> {
    address: *const T,
}

impl<T> Path<T> {
    pub fn new(t: T) -> Self {
        Self {
            address: &t as *const T,
        }
    }
}

// just like that !?
impl<T> Copy for Path<T> {}

impl<T> Clone for Path<T> {
    fn clone(&self) -> Self {
        Self {
            address: self.address,
        }
    }
}

pub struct Symbol {
    hash: u32,
    name: String,
}

impl Symbol {
    pub fn hash(bytes: &[u8]) -> u32 {
        let mut hash = 2166136261u32;
        for byte in bytes.iter() {
            hash ^= *byte as u32;
            hash *= 16777619;
        }
        return hash;
    }
    pub fn take(name: String) -> Self {
        let hash = Symbol::hash(name.as_bytes());
        Self { name, hash }
    }
    pub fn copy(name: &str) -> Self {
        Symbol::take(name.to_string())
    }
}

impl Default for Symbol {
    fn default() -> Self {
        Self::take(Default::default())
    }
}

pub enum Constant {
    Number(f64),
    // just have two types of string. May even use this for the names of methods and classes.
    String(Path<Symbol>),
    Class(Path<Class>),
}

pub enum OpCode {}

pub struct Method {
    pub name: Path<Symbol>,
    pub arity: u16,
    code: Vec<u8>, // cannot just be opcodes.
    lines: Vec<u16>,
}

pub struct Class {
    pub name: Path<Symbol>,
    up_value_count: u16,
    methods: Vec<Method>,
    constant: Vec<Constant>,
}
