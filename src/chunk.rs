use crate::object::Value;

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Op {
    Constant,
    Nil,
    True,
    False,
    Pop,
    GetLocal,
    SetLocal,
    GetGlobal,
    SetGlobal,
    DefineGlobal,
    GetUpvalue,
    SetUpvalue,
    GetProperty,
    SetProperty,
    GetSuper,
    Equal,
    Greater,
    Less,
    Add,
    Subtract,
    Multiply,
    Divide,
    Not,
    Negative,
    Print,
    Jump,
    JumpIfFalse,
    Loop,
    Call,
    Invoke,
    SuperInvoke,
    Closure,
    CloseUpvalue,
    Return,
    Class,
    Inherit,
    Method,
}

const OP_COUNT: usize = Op::Method as usize + 1;
const OP_CODES: [Op; OP_COUNT] = [
    Op::Constant,
    Op::Nil,
    Op::True,
    Op::False,
    Op::Pop,
    Op::GetLocal,
    Op::SetLocal,
    Op::GetGlobal,
    Op::SetGlobal,
    Op::DefineGlobal,
    Op::GetUpvalue,
    Op::SetUpvalue,
    Op::GetProperty,
    Op::SetProperty,
    Op::GetSuper,
    Op::Equal,
    Op::Greater,
    Op::Less,
    Op::Add,
    Op::Subtract,
    Op::Multiply,
    Op::Divide,
    Op::Not,
    Op::Negative,
    Op::Print,
    Op::Jump,
    Op::JumpIfFalse,
    Op::Loop,
    Op::Call,
    Op::Invoke,
    Op::SuperInvoke,
    Op::Closure,
    Op::CloseUpvalue,
    Op::Return,
    Op::Class,
    Op::Inherit,
    Op::Method,
];

impl TryFrom<u8> for Op {
    type Error = String;

    fn try_from(op: u8) -> Result<Self, Self::Error> {
        if op > Op::Method as u8 {
            return Err(format!("{op} is not a valid opcode"));
        }
        Ok(OP_CODES[op as usize])
    }
}

pub struct Chunk {
    pub code: Vec<u8>,
    pub lines: Vec<u16>,
    pub constants: Vec<Value>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            lines: Vec::new(),
            constants: Vec::new(),
        }
    }
    pub fn write(&mut self, bytes: &[u8], line: u16) {
        self.code.extend_from_slice(bytes);
        while self.lines.len() < self.code.len() {
            self.lines.push(line);
        }
    }
    pub fn over_write(&mut self, bytes: &[u8], offset: usize) {
        let end = bytes.len() + offset;
        assert!(end < self.code.len());
        for i in 0..bytes.len() {
            self.code[i] = bytes[i]
        }
    }
    pub fn count(&self) -> usize {
        self.code.len()
    }
    pub fn add_constant(&mut self, value: Value) -> Result<u8, String> {
        let len = self.constants.len();
        if len == u8::MAX as usize {
            Err("too many constants in function".to_string())
        } else {
            self.constants.push(value);
            Ok(len as u8)
        }
    }
}
