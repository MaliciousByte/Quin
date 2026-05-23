pub mod frontend;
pub mod value;
pub mod vm;
pub mod jit;
pub mod stdlib;
pub mod repl;

use frontend::lexer::Lexer;
use frontend::parser::Parser;
use frontend::compiler::Compiler;
use vm::VM;

use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        repl::run_repl();
        return;
    }

    let file_path = &args[1];
    let source = match fs::read_to_string(file_path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Could not read file '{}': {}", file_path, e);
            return;
        }
    };

    let mut lexer = Lexer::new(&source);
    let tokens = match lexer.scan_tokens() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Lexer error: {}", e);
            return;
        }
    };

    let mut parser = Parser::new(tokens);
    let ast = match parser.parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Parser error: {}", e);
            return;
        }
    };

    let compiler = Compiler::new("script", false, false, None);
    let function = match compiler.compile(&ast) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Compiler error: {}", e);
            std::process::exit(1);
        }
    };

    let mut vm = VM::new();
    vm.set_script_dir(file_path);
    if let Err(e) = vm.interpret(function) {
        eprintln!("Runtime error: {}", e);
        std::process::exit(1);
    }
}
