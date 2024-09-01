use std::{env, fs, io, process::exit};

use heap::Heap;

use crate::vm::VM;

#[cfg(feature = "trace")]
mod debug;

#[macro_use]
mod common;
mod bitarray;
mod compiler;
mod functions;
mod op;
mod strings;
// mod loxtr;
// mod table;
mod bound_methods;
mod classes;
mod closures;
mod instances;
mod natives;
mod upvalues;

mod heap;
// mod memory;
mod call_stack;
mod scanner;
mod values;
mod vm;

fn repl(vm: &mut VM) {
    loop {
        print!("> ");
        let mut buf = String::new();
        if io::stdin().read_line(&mut buf).is_err() {
            println!();
            return;
        }
        if buf == "\r\n" {
            println!();
            return;
        }
        if let Err(msg) = vm.interpret(&buf) {
            eprintln!("{}", msg);
        }
    }
}

fn run_file(file_path: &str, vm: &mut VM) {
    let source = fs::read_to_string(file_path)
        .unwrap_or_else(|_| panic!("Couldn't read the file '{}'", file_path));
    if let Err(msg) = vm.interpret(&source) {
        eprintln!("{}", msg);
        exit(70)
    }
}

fn main() {
    let mut vm = VM::new(Heap::new(1 << 12));
    let args: Vec<String> = env::args().collect();
    match args.len() {
        1 => repl(&mut vm),
        2 => run_file(&args[1], &mut vm),
        _ => {
            eprintln!("Usage: rlox [path]\n");
            exit(64);
        }
    }
}
