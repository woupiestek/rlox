use crate::vm::VM;

mod chunk;
mod compiler;
mod memory;
mod object;
mod scanner;
mod stack;
mod vm;

fn main() {
    let vm = VM::new();
    vm.interpret("Hello, world!").unwrap();
}
