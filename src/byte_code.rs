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
    constant_offsets: Vec<u32>,
    functions: Vec<Function>,
}

// one bucket for constants for 2 ** (CONSTANT_SHIFT - 8) instructions
const CONSTANT_SHIFT: usize = 11;

impl ByteCode {
    // it might help to specify some sizes up front, but these 5 array don't all need the same
    pub fn new() -> Self {
        Self {
            code: vec![0],// don't allow writing to ip = 0
            lines: Vec::new(),
            run_lengths: Vec::new(),
            constants: Vec::new(),
            constant_offsets: Vec::new(),
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
    pub fn constant_count(&self) -> usize {
        self.constants.len()
    }

    // mind the offset...
    fn add_constant(&mut self, value: Value) -> Result<(), String> {
        let bucket = self.code.len() >> CONSTANT_SHIFT;
        // empty bucket case
        if bucket >= self.constant_offsets.len() {
            loop {
                self.constant_offsets.push(self.constants.len() as u32);
                if bucket < self.constant_offsets.len() {
                    break;
                }
            }
            self.constants.push(value);
            self.code.push(0);
            return Ok(());
        }
        // search of index case
        let constant_offset = self.constant_offsets[bucket] as usize;
        let l = self.constants.len() - constant_offset;
        for i in 0..l {
            if self.constants[i + constant_offset] == value {
                self.code.push(i as u8);
                return Ok(());
            }
        }
        // can we change the offset of the current bucket?
        // no, the 256 constants in there would be orphaned.
        if l > u8::MAX as usize {
            return err!("Too many constants in function");
        }
        self.constants.push(value);
        self.code.push(l as u8);
        Ok(())
    }

    pub fn write_constant_op(&mut self, op: Op, constant: Value, line: u16) -> Result<(), String> {
        self.code.push(op as u8);
        self.add_constant(constant)?;
        self.put_line(line, 2);
        Ok(())
    }

    pub fn write_byte_op(&mut self, op: Op, byte: u8, line: u16) {
        self.code.push(op as u8);
        self.code.push(byte);
        self.put_line(line, 2);
    }

    pub fn write_invoke_op(
        &mut self,
        op: Op,
        constant: Value,
        arity: u8,
        line: u16,
    ) -> Result<(), String> {
        self.code.push(op as u8);
        self.add_constant(constant)?;
        self.code.push(arity);
        self.put_line(line, 3);
        Ok(())
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

    pub fn read_constant(&self, ip: usize) -> Value {
        let bucket = self.code.len() >> CONSTANT_SHIFT;
        let constant_offset = self.constant_offsets[bucket] as usize;
        self.constants[constant_offset + self.read_byte(ip) as usize]
    }

    #[cfg(feature = "trace")]
    pub fn read_constant_carefully(&self, ip: usize) -> Option<&Value> {
        let bucket = self.code.len() >> CONSTANT_SHIFT;
        let constant_offset = self.constant_offsets[bucket] as usize;
        let index = constant_offset + self.read_byte(ip) as usize;
        self.constants.get(index)
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
