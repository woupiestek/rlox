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
    v_table: [Option<for<'a> fn(&'a mut Disassembler<'hp>) -> ()>; 37],
}

impl<'hp> Disassembler<'hp> {
    pub fn disassemble(heap: &'hp Heap) {
        let mut v_table: [Option<for<'a> fn(&'a mut Disassembler<'hp>) -> ()>; 37] = [None; 37];
        v_table[Op::Call as usize] = Some(Self::byte);
        v_table[Op::Class as usize] = Some(Self::constant);
        v_table[Op::Closure as usize] = Some(Self::constant);
        v_table[Op::Constant as usize] = Some(Self::constant);
        v_table[Op::DefineGlobal as usize] = Some(Self::constant);
        v_table[Op::GetGlobal as usize] = Some(Self::constant);
        v_table[Op::GetLocal as usize] = Some(Self::byte);
        v_table[Op::GetProperty as usize] = Some(Self::constant);
        v_table[Op::GetSuper as usize] = Some(Self::constant);
        v_table[Op::GetUpvalue as usize] = Some(Self::byte);
        v_table[Op::Invoke as usize] = Some(Self::invoke);
        v_table[Op::Jump as usize] = Some(Self::jump_forward);
        v_table[Op::JumpIfFalse as usize] = Some(Self::jump_forward);
        v_table[Op::Loop as usize] = Some(Self::jump_back);
        v_table[Op::Method as usize] = Some(Self::constant);
        v_table[Op::SetGlobal as usize] = Some(Self::constant);
        v_table[Op::SetLocal as usize] = Some(Self::byte);
        v_table[Op::SetProperty as usize] = Some(Self::constant);
        v_table[Op::SetUpvalue as usize] = Some(Self::byte);
        v_table[Op::SuperInvoke as usize] = Some(Self::invoke);

        Self {
            heap,
            fh: FunctionHandle::MAIN,
            ip: 0,
            cp: 0,
            v_table,
        }
        .run();
    }

    fn chunk(&self) -> &Chunk {
        &self.heap.functions.chunk
    }

    fn read_constant(&self) -> Value {
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
            if let Some(f) = self.v_table[op_code as usize] {
                f(self);
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
