use crate::vm::VM;

mod class;
mod heap;
mod object;
mod parser;
mod scanner;
mod stack;
mod string_pool;
mod vm;

fn main() {
    let vm = VM::new();
    vm.interpret("Hello, world!").unwrap();
}
