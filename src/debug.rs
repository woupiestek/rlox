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
            if self.ip >= self.chunk.code.len() {
                return;
            }
            let op_code = match Op::try_from(self.chunk.code[self.ip]) {
                Err(_) => {
                    println!("error: {}", self.chunk.code[self.ip]);
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
                Op::Jump | Op::JumpIfFalse | Op::Loop => self.jump(),
                _ => (),
            }
            println!(";")
        }
    }
    fn byte(&mut self) {
        print!(" {}", self.chunk.code[self.ip]);
        self.ip += 1;
    }
    fn constant(&mut self) {
        let index = self.chunk.code[self.ip];
        let constant = self.chunk.constants[index as usize];
        print!(" {}", constant);
        self.ip += 1;
    }
    fn invoke(&mut self) {
        let constant = self.chunk.code[self.ip];
        let arity = self.chunk.code[self.ip + 1];
        print!(" {} ({})", self.chunk.constants[constant as usize], arity);
        self.ip += 1;
        self.ip += 2
    }
    fn jump(&mut self) {
        self.ip += 2;
        let short =
            ((self.chunk.code[self.ip - 1] as u16) << 8) | (self.chunk.code[self.ip] as u16);
        print!(" {}", short)
    }
}
