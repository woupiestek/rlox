use std::{env, fs, io, process::exit};

use crate::vm::VM;

mod chunk;
mod common;
mod compiler;
mod memory;
mod object;
mod scanner;
mod vm;

fn repl(vm: &mut VM) {
    loop {
        print!("> ");
        let mut buf = String::new();
        if io::stdin().read_line(&mut buf).is_err() {
            print!("\n");
            return;
        }
        if buf == "\r\n" {
            print!("\n");
            return;
        }
        if let Err(msg) = vm.interpret(&buf) {
            eprintln!("{}", msg);
        }
    }
}

fn run_file(file_path: &str, vm: &mut VM) {
    let source =
        fs::read_to_string(file_path).expect(&format!("Couldn't read the file '{}'", file_path));
    if let Err(msg) = vm.interpret(&source) {
        eprintln!("{}", msg);
        exit(70)
    }
}

fn main() {
    let mut vm = VM::new();
    let args: Vec<String> = env::args().collect();
    match args.len() {
        1 => repl(&mut vm),
        2 => run_file(&args[1], &mut vm),
        _ => {
            eprintln!("Usage: clox [path]\n");
            exit(64);
        }
    }
}
