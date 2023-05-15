// byte code data structures

pub struct StaticString {
value: String,
hash: u32
}

pub enum Constant {
    Number(f64),
    // just have two types of string. May even use this for the names of methods and classes.
    String(StaticString),
    Class(Box<Class>),
}

pub enum OpCode {}

pub struct Method {
    pub name: StaticString,
    pub arity: u16,
    code: Vec<u8>, // cannot just be opcodes.
    lines: Vec<u16>,
}

pub struct Class {
    pub name: StaticString,
    up_value_count: u16,
    methods: Vec<Method>,
    constant: Vec<Constant>, 
}
