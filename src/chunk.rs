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

// heap allocated
pub struct Chunk {
    code: Vec<u8>,
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
    pub fn patch_jump(&mut self, offset: usize) -> Result<(), String> {
        assert!({
            let op = self.code[offset - 1];
            op == (Op::Jump as u8) || op == (Op::JumpIfFalse as u8) || op == (Op::Loop as u8)
        });
        let jump = self.code.len() - offset;
        if jump > u16::MAX as usize {
            return err!("Jump too large");
        }
        if jump == 0 {
            return err!("Not a jump");
        }
        self.code[offset] = (jump >> 8) as u8;
        self.code[offset + 1] = jump as u8;
        Ok(())
    }
    pub fn count(&self) -> usize {
        self.code.len()
    }
    pub fn add_constant(&mut self, value: Value) -> Result<u8, String> {
        let mut i = 0;
        while i < self.constants.len() {
            if self.constants[i] == value {
                return Ok(i as u8);
            } else {
                i += 1;
            }
        }
        if i > u8::MAX as usize {
            err!("Too many constants in function")
        } else {
            self.constants.push(value);
            Ok(i as u8)
        }
    }

    pub fn write_byte_op(&mut self, op: Op, byte: u8, line: u16) {
        self.code.push(op as u8);
        self.code.push(byte);
        self.lines.push(line);
        self.lines.push(line);
    }
    pub fn write_invoke_op(&mut self, op: Op, constant: u8, arity: u8, line: u16) {
        self.code.push(op as u8);
        self.code.push(constant);
        self.code.push(arity);
        self.lines.push(line);
        self.lines.push(line);
        self.lines.push(line);
    }
    pub fn write_short_op(&mut self, op: Op, short: u16, line: u16) {
        self.code.push(op as u8);
        self.code.push((short >> 8) as u8);
        self.code.push(short as u8);
        self.lines.push(line);
        self.lines.push(line);
        self.lines.push(line);
    }

    pub fn read_byte(&self, index: usize) -> u8 {
        self.code[index]
    }
    pub fn read_short(&self, index: usize) -> u16 {
        (self.read_byte(index) as u16) << 8 | (self.read_byte(index + 1) as u16)
    }
    pub fn read_constant(&self, index: usize) -> Value {
        self.constants[self.read_byte(index) as usize]
    }
    // count adjustment after compiling
    pub fn byte_increment(&self) -> usize {
        self.code.capacity() + 2 * self.lines.capacity() + 2 * self.constants.capacity()
    }
}
