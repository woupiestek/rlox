use crate::{chunk::Op, object::Value, strings::StringHandle};

// still over allocating because of alignment!
// o/c if we knew how small the offsets could actually be...
// todo: not all are mutable, so why provide mutable references?
#[derive(Debug)]
pub struct Function {
    pub name: StringHandle, // run time data structure
    pub arity: u8,
    pub upvalue_count: u8,
    pub ip: u32,
    pub constant_offset: u32,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FunctionHandle(u16);

impl FunctionHandle {
    pub const MAIN: Self = Self(0);
}

#[derive(Debug)]
pub struct ByteCode {
    code: Vec<u8>,
    lines: Vec<u16>,
    run_lengths: Vec<u16>,
    constants: Vec<Value>, // run time data structure
    functions: Vec<Function>,
}

impl ByteCode {
    // it might help to specify some sizes up front, but these 5 array don't all need the same
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            lines: Vec::new(),
            run_lengths: Vec::new(),
            constants: Vec::new(),
            functions: Vec::new(),
        }
    }

    // repo pattern
    pub fn new_function(&mut self, name: Option<StringHandle>) -> FunctionHandle {
        self.functions.push(Function {
            name: name.unwrap_or(StringHandle::EMPTY), // be careful with this!
            arity: 0,
            upvalue_count: 0,
            ip: self.code.len() as u32,
            constant_offset: self.constants.len() as u32,
        });
        FunctionHandle((self.functions.len() - 1) as u16)
    }

    pub fn function_ref(&self, fi: FunctionHandle) -> &Function {
        &self.functions[fi.0 as usize]
    }

    pub fn function_mut(&mut self, fi: FunctionHandle) -> &mut Function {
        &mut self.functions[fi.0 as usize]
    }

    fn put_line(&mut self, line: u16, run_length: u16) {
        if self.lines.len() > 0 {
            let index = self.lines.len() - 1;
            if self.lines[index] == line {
                self.run_lengths[index] += run_length;
                return;
            }
        }
        self.lines.push(line);
        self.run_lengths.push(run_length);
    }

    pub fn get_line(&self, ip: u32) -> u16 {
        let mut run_length: u32 = 0;
        for i in 0..self.lines.len() {
            run_length += self.run_lengths[i] as u32;
            if run_length > ip {
                return self.lines[i];
            }
        }
        return 0;
    }

    pub fn write(&mut self, bytes: &[u8], line: u16) {
        self.code.extend_from_slice(bytes);
        self.put_line(line, bytes.len() as u16);
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

    // note the offset...
    pub fn add_constant(&mut self, fh: FunctionHandle, value: Value) -> Result<u8, String> {
        let constant_offset = self.functions[fh.0 as usize].constant_offset;
        let l = self.constants.len() as u32;
        let mut i = constant_offset;
        while i < l {
            if self.constants[i as usize] == value {
                return Ok((i - constant_offset) as u8);
            } else {
                i += 1;
            }
        }
        if i - constant_offset > u8::MAX as u32 {
            err!("Too many constants in function")
        } else {
            self.constants.push(value);
            Ok((i - constant_offset) as u8)
        }
    }

    pub fn write_byte_op(&mut self, op: Op, byte: u8, line: u16) {
        self.code.push(op as u8);
        self.code.push(byte);
        self.put_line(line, 2);
    }
    pub fn write_invoke_op(&mut self, op: Op, constant: u8, arity: u8, line: u16) {
        self.code.push(op as u8);
        self.code.push(constant);
        self.code.push(arity);
        self.put_line(line, 3);
    }
    pub fn write_short_op(&mut self, op: Op, short: u16, line: u16) {
        self.code.push(op as u8);
        self.code.push((short >> 8) as u8);
        self.code.push(short as u8);
        self.put_line(line, 3);
    }
    pub fn read_byte(&self, index: usize) -> u8 {
        self.code[index]
    }
    pub fn read_short(&self, index: usize) -> u16 {
        (self.read_byte(index) as u16) << 8 | (self.read_byte(index + 1) as u16)
    }

    // this could be the point
    pub fn read_constant(&self, function: FunctionHandle, ip: usize) -> Value {
        // needs function
        self.constants[self.functions[function.0 as usize].constant_offset as usize
            + self.read_byte(ip) as usize]
    }

    // we are moving toward not using the garbage collector for static data
    // here is why: this method does the same thing on every cycle.
    pub fn trace(
        &self,
        collector: &mut Vec<crate::heap::Handle>,
        strings: &mut Vec<crate::strings::StringHandle>,
    ) {
        for f in &self.functions {
            if f.name != StringHandle::EMPTY {
                strings.push(f.name);
            }
        }
        for value in &self.constants {
            if let &Value::Object(h) = value {
                collector.push(h)
            }
            if let &Value::String(h) = value {
                strings.push(h)
            }
        }
    }
}
