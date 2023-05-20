use crate::vm::VM;

mod class;
mod common;
mod compiler;
mod memory;
mod object;
mod scanner;
mod stack;
mod string_pool;
mod vm;

fn main() {
    let vm = VM::new();
    vm.interpret("Hello, world!").unwrap();
}
