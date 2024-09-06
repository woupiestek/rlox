use crate::{
    bitarray::BitArray,
    heap::{Collector, Handle, Heap, Pool, FUNCTION},
    op::Op,
    strings::StringHandle,
    values::Value,
};

#[derive(Debug)]
pub struct Chunk {
    code: Vec<u8>,
    lines: Vec<u16>,
    run_lengths: Vec<u16>,
    constants: Vec<Value>, // run time data structure
}

impl Chunk {
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

    pub fn get_line(&self, ip: i32) -> u16 {
        let mut run_length: i32 = 0;
        for i in 0..self.lines.len() {
            run_length += self.run_lengths[i] as i32;
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
    pub fn ip(&self) -> usize {
        self.code.len()
    }

    // mind the offset...
    fn add_constant(&mut self, value: Value) -> Result<(), String> {
        let l = self.constants.len();
        for i in 0..l {
            if self.constants[i] == value {
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
        self.constants[self.read_byte(ip) as usize]
    }
}

pub type FunctionHandle = Handle<FUNCTION>;

impl FunctionHandle {
    pub const MAIN: Self = Self(0);
}

#[derive(Debug)]
pub struct Functions {
    names: Vec<StringHandle>, // run time data structure
    arities: Vec<u8>,
    upvalue_counts: Vec<u8>,
    chunks: Vec<Chunk>,
}

impl Functions {
    // it might help to specify some sizes up front, but these 5 arrays don't all need the same
    pub fn new() -> Self {
        Self {
            names: Vec::new(), // run time data structure
            arities: Vec::new(),
            upvalue_counts: Vec::new(),
            chunks: Vec::new(),
        }
    }

    // repo pattern
    pub fn new_function(&mut self, name: Option<StringHandle>) -> FunctionHandle {
        self.arities.push(0);
        self.chunks.push(Chunk {
            code: Vec::new(),
            lines: Vec::new(),
            run_lengths: Vec::new(),
            constants: Vec::new(),
        });
        self.names.push(name.unwrap_or(StringHandle::EMPTY));
        self.upvalue_counts.push(0);
        FunctionHandle::from((self.chunks.len() - 1) as u32)
    }

    pub fn chunk_ref(&self, fh: FunctionHandle) -> &Chunk {
        &self.chunks[fh.index()]
    }

    pub fn chunk_mut(&mut self, fh: FunctionHandle) -> &mut Chunk {
        &mut self.chunks[fh.index()]
    }

    pub fn incr_arity(&mut self, fh: FunctionHandle) -> Result<(), String> {
        if self.arities[fh.index()] == u8::MAX {
            return err!("Can't have more than 255 parameters.");
        }
        self.arities[fh.index()] += 1;
        Ok(())
    }

    pub fn arity(&self, fh: FunctionHandle) -> u8 {
        self.arities[fh.index()]
    }

    pub fn set_upvalue_count(&mut self, fh: FunctionHandle, count: u8) {
        self.upvalue_counts[fh.index()] = count
    }

    pub fn upvalue_count(&self, fh: FunctionHandle) -> usize {
        self.upvalue_counts[fh.index()] as usize
    }

    #[cfg(feature = "trace")]
    pub fn count(&self) -> usize {
        self.chunks.len()
    }

    pub fn to_string(&self, fh: FunctionHandle, heap: &Heap) -> String {
        let i = fh.0 as usize;
        let name = self.names[i];
        if name == StringHandle::EMPTY {
            format!("<script>")
        } else {
            format!(
                "<fn {} ({}/{})>",
                heap.strings.get(name).unwrap(),
                self.arities[i],
                self.upvalue_counts[i]
            )
        }
    }
}

impl Pool<FUNCTION> for Functions {
    fn byte_count(&self) -> usize {
        // replace with more realstic number
        self.names.capacity() * 102
    }

    fn count(&self) -> usize {
        self.names.len()
    }

    fn trace(&self, handle: Handle<FUNCTION>, collector: &mut Collector) {
        if self.names[handle.index()] != StringHandle::EMPTY {
            collector.keys.push(self.names[handle.index()])
        }
        for constant in &self.chunks[handle.index()].constants {
            constant.trace(collector)
        }
    }

    fn sweep(&mut self, marks: &BitArray) {
        for i in 0..self.count() {
            if !marks.has(i) {
                self.names[i] = StringHandle::EMPTY;
                self.arities[i] = 0;
                self.chunks[i].code.clear();
                self.chunks[i].constants.clear();
                self.chunks[i].lines.clear();
                self.chunks[i].run_lengths.clear();
            }
        }
    }
}
