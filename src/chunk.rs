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
    Not,
    Negative,
    JumpIfFalse,
    Loop,
    Call,
    Invoke,
    SuperInvoke,
    CloseUpvalue,
    Return,
}

pub struct Chunk {
    code: Vec<u8>,
    lines: Vec<u16>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            lines: Vec::new(),
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
}
