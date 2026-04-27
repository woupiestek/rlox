use std::mem;

use crate::{
    handles::{Column, HandleSet},
    heap::{Collector, Handle, Heap, Pool, Traceable, FUNCTION},
    symbols::SymbolHandle,
    values::Value,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
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
    constants: Vec<Value>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            lines: Vec::new(),
            run_lengths: Vec::new(),
            constants: Vec::new(),
        }
    }
    pub fn add(&mut self, cd: &[u8], ln: &[u16], rl: &[u16], cn: &[Value]) -> ChunkFrame {
        let frame = ChunkFrame {
            ip: self.code.len(),
            lp: self.lines.len(),
            cp: self.constants.len(),
        };
        self.code.extend_from_slice(cd);
        self.lines.extend_from_slice(ln);
        self.run_lengths.extend_from_slice(rl);
        self.constants.extend_from_slice(cn);
        frame
    }

    // not producing correct line numbers.
    pub fn get_line(&self, frame: ChunkFrame, ip: usize) -> u16 {
        // start from start of frame, not from the beginning!
        let mut run_length = self.run_lengths[frame.lp] as usize;
        for lp in frame.lp..self.run_lengths.len() {
            run_length += self.run_lengths[lp] as usize;
            if run_length > ip {
                return self.lines[lp];
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
    names: Column<SymbolHandle>,
    arities: Column<u8>,
    upvalue_counts: Column<u8>,
    frames: Column<ChunkFrame>,
    pub chunk: Chunk,
    handles: HandleSet,
}

impl Functions {
    // it might help to specify some sizes up front, but these 5 arrays don't all need the same
    pub fn new() -> Self {
        Self {
            names: Column::new(),
            arities: Column::new(),
            upvalue_counts: Column::new(),
            frames: Column::new(),
            chunk: Chunk::new(),
            handles: HandleSet::new(),
        }
    }

    // repo pattern
    pub fn new_function(
        &mut self,
        name: Option<SymbolHandle>,
        arity: u8,
        upvalue_count: u8,
        frame: ChunkFrame,
    ) -> FunctionHandle {
        let i = self.handles.next();
        self.arities.set(i, arity);
        self.frames.set(i, frame);
        self.names.set(i, name.unwrap_or(SymbolHandle::EMPTY));
        self.upvalue_counts.set(i, upvalue_count);
        FunctionHandle::from(i)
    }

    pub fn arity(&self, fh: FunctionHandle) -> u8 {
        self.arities.get(fh.0)
    }

    pub fn upvalue_count(&self, fh: FunctionHandle) -> usize {
        self.upvalue_counts.get(fh.0) as usize
    }

    pub fn get_frame(&self, fh: FunctionHandle) -> ChunkFrame {
        self.frames.get(fh.0)
    }

    #[cfg(feature = "trace")]
    pub fn count(&self) -> usize {
        self.chunk.frames.len()
    }

    pub fn to_string(&self, fh: FunctionHandle, heap: &Heap) -> String {
        let name = self.names.get(fh.0);
        if name == SymbolHandle::EMPTY {
            format!("<script>")
        } else {
            format!(
                "<fn {} ({}/{})>",
                heap.symbols.get(name),
                self.arities.get(fh.0),
                self.upvalue_counts.get(fh.0)
            )
        }
    }

    pub fn get_line(&self, fh: FunctionHandle, ip: usize) -> u16 {
        self.chunk.get_line(self.frames.get(fh.0), ip)
    }
}

impl Pool<FUNCTION> for Functions {
    fn byte_count(&self) -> usize {
        // replace with more realistic number
        self.names.byte_count()
            + self.arities.byte_count()
            + self.frames.byte_count()
            + self.handles.byte_count()
            + self.upvalue_counts.byte_count()
            + mem::size_of::<Self>()
    }

    fn mark(&mut self, handle: u32) -> bool {
        self.handles.mark(handle)
    }

    fn trace_all(&mut self, marked: &Vec<u32>, collector: &mut Collector) {
        for &i in marked {
            let name = self.names.get(i);
            if name != SymbolHandle::EMPTY {
                name.trace(collector);
            }
        }
        for &i in marked {
            // the length of the constant array is no longe recorded
            // this only works because no garbage is actually collected here
            // if function memory is not managed that way,
            // then we should just get rid of this pool.
            let from = self.get_frame(Handle(i)).cp;
            let next = self.get_frame(Handle(i + 1)).cp;
            let to = if next == 0 {
                self.chunk.constants.len()
            } else {
                next
            };
            for constant in from..to {
                self.chunk.constants[constant].trace(collector)
            }
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
