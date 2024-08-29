use crate::{
    functions::{Chunk, FunctionHandle, Functions}, heap::Heap, op::Op
};

pub struct Disassembler<'src, 'hp> {
    functions: &'src Functions,
    heap: &'hp Heap,
    fh: FunctionHandle,
    ip: usize,
}

impl<'src, 'hp> Disassembler<'src, 'hp> {
    pub fn disassemble(functions: &'src Functions, heap: &'hp Heap) {
        Self {
            functions,
            heap,
            fh: FunctionHandle::MAIN,
            ip: 0,
        }.run();
    }

    fn chunk(&self) -> &Chunk {
        self.functions.chunk_ref(self.fh)
    }

    fn run(&mut self) {
        for i in 0..self.functions.count() {
            self.fh = FunctionHandle::from_index(i);
            println!("{}:", self.functions.to_string(self.fh, self.heap));
            self.ip = 0;
            self.code();
        }
    }

    fn code(&mut self) {
        while self.ip < self.chunk().ip() {
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
        let value = self.chunk().read_constant(self.ip);
        print!(" {}", value.to_string(&self.heap, &self.functions));
        self.ip += 1;
    }
    fn invoke(&mut self) {
        print!(
            " {} ({})",
            self.chunk()
                .read_constant(self.ip)
                .to_string(&self.heap, &self.functions),
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
