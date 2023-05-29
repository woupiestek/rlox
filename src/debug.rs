use crate::chunk::{Chunk, Op};

pub struct Disassembler<'src> {
    chunk: &'src Chunk,
    ip: usize,
}

impl<'src> Disassembler<'src> {
    fn new(chunk: &'src Chunk) -> Self {
        Self { chunk, ip: 0 }
    }

    pub fn disassemble(chunk: &'src Chunk) {
        Self::new(chunk).run();
    }

    fn run(&mut self) {
        loop {
            if self.ip >= self.chunk.count() {
                return;
            }
            print!("{}:", self.ip);
            let op_code = match Op::try_from(self.chunk.read_byte(self.ip)) {
                Err(_) => {
                    println!("error: {}", self.chunk.read_byte(self.ip));
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
        print!(" {}", self.chunk.read_byte(self.ip));
        self.ip += 1;
    }
    fn constant(&mut self) {
        print!(" {}", self.chunk.read_constant(self.ip));
        self.ip += 1;
    }
    fn invoke(&mut self) {
        print!(
            " {} ({})",
            self.chunk.read_constant(self.ip),
            self.chunk.read_byte(self.ip + 1)
        );
        self.ip += 2;
    }
    fn jump_forward(&mut self) {
        print!(" {}", self.ip + self.chunk.read_short(self.ip) as usize);
        self.ip += 2;
    }
    fn jump_back(&mut self) {
        print!(" {}", self.ip - self.chunk.read_short(self.ip) as usize);
        self.ip += 2;
    }
}
