use std::ops::Range;

use crate::{
    handles::Handles,
    heap::{Collector, Handle, Heap, Pool, Traceable, FUNCTION},
    strings::StringHandle,
    values::Value,
};

#[derive(Debug)]
pub struct ChunkFrame {
    pub ip: usize,
    pub lp: usize,
    pub cp: usize,
}

#[derive(Debug)]
pub struct Chunk {
    code: Vec<u8>,
    lines: Vec<u16>,
    run_lengths: Vec<u16>,
    constants: Vec<Value>,   // run time data structure
    frames: Vec<ChunkFrame>, //
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            lines: Vec::new(),
            run_lengths: Vec::new(),
            constants: Vec::new(),
            frames: Vec::new(),
        }
    }
    pub fn add(&mut self, cd: &[u8], ln: &[u16], rl: &[u16], cn: &[Value]) -> usize {
        let len = self.frames.len();
        self.frames.push(ChunkFrame {
            ip: self.code.len(),
            lp: self.lines.len(),
            cp: self.constants.len(),
        });
        self.code.extend_from_slice(cd);
        self.lines.extend_from_slice(ln);
        self.run_lengths.extend_from_slice(rl);
        self.constants.extend_from_slice(cn);
        len
    }

    // the problem case
    pub fn get_line(&self, frame: usize, ip: usize) -> u16 {
        // start from start of frame, not from the beginning!
        let mut run_length: usize = self.frames[frame].lp;

        let i0 = self.frames[frame].lp;
        let l = self.lines.len();
        let i1 = if frame + 1 == l {
            self.frames.len()
        } else {
            self.frames[frame + 1].lp
        };
        for i in i0..i1 {
            run_length += self.run_lengths[i] as usize;
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
    pub fn read_constant(&self, cp: usize) -> Value {
        self.constants[cp]
    }

    #[cfg(feature = "trace")]
    pub fn len(&self) -> usize {
        self.code.len()
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
    frames: Vec<usize>,
    pub chunk: Chunk,
    handles: Handles,
}

impl Functions {
    // it might help to specify some sizes up front, but these 5 arrays don't all need the same
    pub fn new() -> Self {
        Self {
            names: Vec::new(), // run time data structure
            arities: Vec::new(),
            upvalue_counts: Vec::new(),
            // indirection to allow compactification...
            frames: Vec::new(),
            chunk: Chunk::new(),
            handles: Handles::new(),
        }
    }

    // repo pattern
    pub fn new_function(
        &mut self,
        name: Option<StringHandle>,
        arity: u8,
        upvalue_count: u8,
        frame: usize,
    ) -> FunctionHandle {
        let i = self.handles.next();
        while i as usize >= self.arities.len() {
            self.arities.push(arity);
            self.frames.push(frame);
            self.names.push(name.unwrap_or(StringHandle::EMPTY));
            self.upvalue_counts.push(upvalue_count);
        }
        FunctionHandle::from(i)
    }

    pub fn arity(&self, fh: FunctionHandle) -> u8 {
        self.arities[fh.index()]
    }

    pub fn upvalue_count(&self, fh: FunctionHandle) -> usize {
        self.upvalue_counts[fh.index()] as usize
    }

    pub fn get_frame(&self, fh: FunctionHandle) -> &ChunkFrame {
        &self.chunk.frames[self.frames[fh.index()]]
    }

    pub fn constants(&self, fh: FunctionHandle) -> Range<usize> {
        let frame = self.frames[fh.index()];
        let from = self.chunk.frames[frame].cp;
        let len = self.chunk.frames.len();
        let to = if frame + 1 == len {
            self.chunk.constants.len()
        } else {
            self.chunk.frames[frame + 1].cp
        };
        from..to
    }

    #[cfg(feature = "trace")]
    pub fn count(&self) -> usize {
        self.chunk.frames.len()
    }

    pub fn to_string(&self, fh: FunctionHandle, heap: &Heap) -> String {
        let i = fh.0 as usize;
        let name = self.names[i];
        if name == StringHandle::EMPTY {
            format!("<script>")
        } else {
            format!(
                "<fn {} ({}/{})>",
                heap.strings.get(name),
                self.arities[i],
                self.upvalue_counts[i]
            )
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
            self.names[handle.index()].trace(collector);
        }
        for constant in self.constants(handle) {
            self.chunk.constants[constant].trace(collector)
        }
    }

    fn reset(&mut self) {
        self.handles.clear();
    }

    fn sweep(&mut self) {
        // compaction:
        // if the number of frames is (much) larger than the number of handles
        // then it may be useful to start moving stuff around.
    }
}
