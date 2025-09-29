use crate::{
    functions::{Chunk, FunctionHandle},
    heap::Heap,
    op::Op,
    values::Value,
};

pub struct Disassembler<'hp> {
    heap: &'hp Heap,
    fh: FunctionHandle,
    ip: usize,
    cp: usize,
}

impl<'hp> Disassembler<'hp> {
    pub fn disassemble(heap: &'hp Heap) {
        Self {
            heap,
            fh: FunctionHandle::MAIN,
            ip: 0,
            cp: 0,
        }
        .run();
    }

    fn chunk(&self) -> &Chunk {
        &self.heap.functions.chunk
    }

    fn read_constant(&self) -> Value {
        // fuck!
        self.chunk()
            .read_constant(self.cp + self.chunk().read_byte(self.ip) as usize)
    }

    fn run(&mut self) {
        let l = self.heap.functions.count();
        for i in 0..l {
            self.fh = FunctionHandle::from(i as u32);
            println!("{}:", self.heap.functions.to_string(self.fh, self.heap));
            let frame = self.heap.functions.get_frame(self.fh);
            self.cp = frame.cp;
            self.ip = frame.ip;
            let end = if i + 1 < l {
                self.heap
                    .functions
                    .get_frame(FunctionHandle::from(i as u32 + 1))
                    .ip
            } else {
                self.chunk().len()
            };
            self.code(end);
        }
    }

    fn code(&mut self, len: usize) {
        while self.ip < len {
            print!("{}:", self.ip);
            let op_code = match Op::try_from(self.chunk().read_byte(self.ip)) {
                Err(_) => {
                    println!("error: {}", self.chunk().read_byte(self.ip));
                    self.ip += 1;
                    continue;
                }
                Ok(op_code) => {
                    print!("{:?}", op_code);
                    self.ip += 1;
                    op_code
                }
            };
            match op_code {
                Op::Call | Op::GetLocal | Op::GetUpvalue | Op::SetLocal | Op::SetUpvalue => {
                    self.byte()
                }
                Op::Class
                | Op::Closure
                | Op::Constant
                | Op::DefineGlobal
                | Op::GetGlobal
                | Op::GetProperty
                | Op::GetSuper
                | Op::Method
                | Op::SetGlobal
                | Op::SetProperty => self.constant(),
                Op::Invoke | Op::SuperInvoke => self.invoke(),
                Op::Jump | Op::JumpIfFalse => self.jump_forward(),
                Op::Loop => self.jump_back(),
                _ => (),
            }
            println!(";")
        }
    }
    fn byte(&mut self) {
        print!(" {}", self.chunk().read_byte(self.ip));
        self.ip += 1;
    }
    fn constant(&mut self) {
        let value = self.read_constant();
        print!(" {}", value.to_string(&self.heap));
        self.ip += 1;
    }
    fn invoke(&mut self) {
        print!(
            " {} ({})",
            self.read_constant().to_string(&self.heap),
            self.chunk().read_byte(self.ip + 1)
        );
        self.ip += 2;
    }
    fn jump_forward(&mut self) {
        print!(" {}", self.ip + self.chunk().read_short(self.ip) as usize);
        self.ip += 2;
    }
    fn jump_back(&mut self) {
        print!(" {}", self.ip - self.chunk().read_short(self.ip) as usize);
        self.ip += 2;
    }
}
