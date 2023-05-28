use crate::vm::VM;

mod chunk;
mod common;
mod compiler;
mod memory;
mod object;
mod scanner;
mod vm;

fn main() {
    let mut vm = VM::new();
    if let Err(msg) = vm.interpret("Hello, world!") {
        println!("{}", msg);
    }
}
