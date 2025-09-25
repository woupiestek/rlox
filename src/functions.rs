use crate::{
    handles::Handles,
    heap::{Collector, Handle, Heap, Pool, FUNCTION},
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
    pub fn fill(&mut self, cd: &[u8], ln: &[u16], rl: &[u16], cn: &[Value]) {
        self.code.extend_from_slice(cd);
        self.lines.extend_from_slice(ln);
        self.run_lengths.extend_from_slice(rl);
        self.constants.extend_from_slice(cn);
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

pub struct Functions {
    names: Vec<StringHandle>, // run time data structure
    arities: Vec<u8>,
    upvalue_counts: Vec<u8>,
    chunks: Vec<Chunk>,
    handles: Handles,
}

impl Functions {
    // it might help to specify some sizes up front, but these 5 arrays don't all need the same
    pub fn new() -> Self {
        Self {
            names: Vec::new(), // run time data structure
            arities: Vec::new(),
            upvalue_counts: Vec::new(),
            chunks: Vec::new(),
            handles: Handles::new(),
        }
    }

    // repo pattern
    pub fn new_function(
        &mut self,
        name: Option<StringHandle>,
        arity: u8,
        upvalue_count: u8,
    ) -> FunctionHandle {
        let i = self.handles.next();
        while i as usize >= self.arities.len() {
            self.arities.push(arity);
            self.chunks.push(Chunk {
                code: Vec::new(),
                lines: Vec::new(),
                run_lengths: Vec::new(),
                constants: Vec::new(),
            });
            self.names.push(name.unwrap_or(StringHandle::EMPTY));
            self.upvalue_counts.push(upvalue_count);
        }
        FunctionHandle::from(i)
    }

    pub fn chunk_ref(&self, fh: FunctionHandle) -> &Chunk {
        &self.chunks[fh.index()]
    }

    pub fn chunk_mut(&mut self, fh: FunctionHandle) -> &mut Chunk {
        &mut self.chunks[fh.index()]
    }

    pub fn arity(&self, fh: FunctionHandle) -> u8 {
        self.arities[fh.index()]
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

    pub fn sweep(&mut self) {
        for i in 0..self.names.len() {
            if !self.handles.is_marked(i as u32) {
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

impl Pool<FUNCTION> for Functions {
    fn byte_count(&self) -> usize {
        // replace with more realistic number
        self.names.capacity() * 96
    }

    fn trace(&mut self, handle: Handle<FUNCTION>, collector: &mut Collector) {
        if !self.handles.mark(handle.0) {
            return;
        }
        if self.names[handle.index()] != StringHandle::EMPTY {
            collector.push(self.names[handle.index()])
        }
        for constant in &self.chunks[handle.index()].constants {
            constant.trace(collector)
        }
    }

    fn reset(&mut self) {
        self.handles.clear();
    }

    fn sweep(&mut self) {}
}
