use crate::{
    byte_code::{ByteCode, FunctionHandle},
    chunk::Op,
    heap::Heap,
};

pub struct Disassembler<'src, 'hp> {
    byte_code: &'src ByteCode,
    heap: &'hp Heap,
    ip: usize,
}

impl<'src, 'hp> Disassembler<'src, 'hp> {
    fn new(byte_code: &'src ByteCode, heap: &'hp Heap) -> Self {
        Self {
            byte_code,
            heap,
            ip: 1, // skip first byte!
        }
    }

    pub fn disassemble(byte_code: &'src ByteCode, heap: &'hp Heap) {
        Self::new(byte_code, heap).run();
    }

    fn run(&mut self) {
        self.functions();
    }

    fn code(&mut self, to: usize) {
        while self.ip < to {
            print!("{}:", self.ip);
            let op_code = match Op::try_from(self.byte_code.read_byte(self.ip)) {
                Err(_) => {
                    println!("error: {}", self.byte_code.read_byte(self.ip));
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
        print!(" {}", self.byte_code.read_byte(self.ip));
        self.ip += 1;
    }
    fn constant(&mut self) {
        let value = self.byte_code.read_constant(self.ip);
        print!(" {}", value.to_string(&self.heap, &self.byte_code));
        self.ip += 1;
    }
    fn invoke(&mut self) {
        print!(
            " {} ({})",
            self.byte_code
                .read_constant(self.ip)
                .to_string(&self.heap, &self.byte_code),
            self.byte_code.read_byte(self.ip + 1)
        );
        self.ip += 2;
    }
    fn jump_forward(&mut self) {
        print!(" {}", self.ip + self.byte_code.read_short(self.ip) as usize);
        self.ip += 2;
    }
    fn jump_back(&mut self) {
        print!(" {}", self.ip - self.byte_code.read_short(self.ip) as usize);
        self.ip += 2;
    }

    fn functions(&mut self) {
        let l = self.byte_code.function_count();
        for i in 0..l {
            let current = FunctionHandle::from_index(i);
            println!("{}:", self.byte_code.function_string(current, &self.heap));
            self.ip = self
                .byte_code
                .function_ref(current)
                .ip as usize;
            let to = if i + 1 == l {
                self.byte_code.count()
            } else {
                self.byte_code
                    .function_ref(FunctionHandle::from_index(i + 1))
                    .ip as usize
            };
            self.code(to);
        }
    }
}
