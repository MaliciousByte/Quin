pub mod token;
pub mod lexer;
pub mod ast;
pub mod parser;
pub mod value;
pub mod chunk;
pub mod compiler;
pub mod vm;

use lexer::Lexer;
use parser::Parser;
use compiler::Compiler;
use vm::VM;

use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: quin <file.qn>");
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

    let compiler = Compiler::new("script", false, false);
    let function = match compiler.compile(&ast) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Compiler error: {}", e);
            return;
        }
    };

    let mut vm = VM::new();
    if let Err(e) = vm.interpret(function) {
        eprintln!("Runtime error: {}", e);
    }
}
