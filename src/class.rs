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
        *self
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
            hash = hash.wrapping_mul(16777619);
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

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum OpCode {
    Return,
}
impl TryFrom<u8> for OpCode {
    type Error = String;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        // keep updating...
        const OP_CODES: [OpCode; 1] = [OpCode::Return];
        match OP_CODES.get(value as usize) {
            Some(op_code) => Ok(*op_code),
            None => Err(format!("unknown upcode {value}")),
        }
    }
}

pub struct Method {
    pub name: Path<Symbol>,
    pub arity: u16,
    code: Vec<u8>, // cannot just be opcodes.
    lines: Vec<u16>,
}

impl Method {
    pub fn write(&mut self, byte: u8, line: u16) {
        self.code.push(byte);
        self.lines.push(line);
    }
}

pub struct Class {
    pub name: Path<Symbol>,
    up_value_count: u16,
    methods: Vec<Method>,
    constant: Vec<Constant>,
}
