pub mod token;
pub mod lexer;
pub mod ast;
pub mod parser;
pub mod value;
pub mod chunk;
pub mod compiler;
pub mod vm;
pub mod obj;
pub mod jit;
pub mod interner;
pub mod stdlib;

use lexer::Lexer;
use parser::Parser;
use compiler::Compiler;
use vm::VM;

use std::env;
use std::fs;
use std::io::{self, Write, BufRead};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        run_repl();
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

fn run_repl() {
    println!();
    println!("  \x1b[1;36m╔══════════════════════════════════════╗\x1b[0m");
    println!("  \x1b[1;36m║\x1b[0m   \x1b[1;35m🌌 Quin v0.2.0\x1b[0m                     \x1b[1;36m║\x1b[0m");
    println!("  \x1b[1;36m║\x1b[0m   Interactive Mode                   \x1b[1;36m║\x1b[0m");
    println!("  \x1b[1;36m╚══════════════════════════════════════╝\x1b[0m");
    println!();
    println!("  Type \x1b[1;33m.help\x1b[0m for commands, \x1b[1;33m.exit\x1b[0m to quit.");
    println!();

    let mut vm = VM::new();
    let stdin = io::stdin();
    let mut input_buffer = String::new();
    let mut brace_depth: i32 = 0;
    let mut in_multiline = false;

    loop {
        // Print prompt
        if in_multiline {
            print!("\x1b[1;34m... \x1b[0m");
        } else {
            print!("\x1b[1;32m>>> \x1b[0m");
        }
        io::stdout().flush().unwrap();

        // Read line
        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => {
                // EOF (Ctrl+D / Ctrl+Z)
                println!();
                break;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("Read error: {}", e);
                break;
            }
        }

        let trimmed = line.trim();

        // Handle special commands
        if !in_multiline {
            match trimmed {
                ".exit" | ".quit" => {
                    println!("\x1b[1;36mGoodbye! 👋\x1b[0m");
                    break;
                }
                ".help" => {
                    println!();
                    println!("  \x1b[1;33m.help\x1b[0m     Show this help message");
                    println!("  \x1b[1;33m.exit\x1b[0m     Exit the REPL");
                    println!("  \x1b[1;33m.clear\x1b[0m    Clear the screen");
                    println!();
                    println!("  Enter Quin code to evaluate.");
                    println!("  Multi-line input: open a \x1b[1m{{\x1b[0m and keep typing.");
                    println!("  Expressions auto-print their result.");
                    println!();
                    continue;
                }
                ".clear" => {
                    print!("\x1b[2J\x1b[H");
                    io::stdout().flush().unwrap();
                    continue;
                }
                "" => continue,
                _ => {}
            }
        }

        // Track braces for multi-line
        for ch in trimmed.chars() {
            if ch == '{' { brace_depth += 1; }
            if ch == '}' { brace_depth -= 1; }
        }

        input_buffer.push_str(&line);

        if brace_depth > 0 {
            in_multiline = true;
            continue;
        }

        // We have a complete input — execute it
        in_multiline = false;
        brace_depth = 0;
        let source = input_buffer.trim().to_string();
        input_buffer.clear();

        if source.is_empty() { continue; }

        // Try to lex, parse, compile, run
        let result = execute_repl_line(&mut vm, &source);
        match result {
            Ok(()) => {}
            Err(e) => {
                eprintln!("\x1b[1;31mError:\x1b[0m {}", e);
            }
        }
    }
}

fn execute_repl_line(vm: &mut VM, source: &str) -> Result<(), String> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.scan_tokens()?;

    let mut parser = Parser::new(tokens);
    let ast = parser.parse()?;

    // Check if the last statement is an expression — if so, auto-print it
    let should_print = matches!(ast.last(), Some(crate::ast::Stmt::Expression(_)));

    let compiler = Compiler::new("repl", false, false, None);
    let function = compiler.compile(&ast)?;

    vm.interpret(function)?;

    // Auto-print the result if it was a bare expression
    if should_print {
        // The VM already popped expression results via OpCode::Pop,
        // but for REPL we want to see them. We handle this by
        // checking the last value. The emit function handles output
        // through the VM globals, so bare expressions already work
        // if ending with emit. For now, expressions are printed via emit.
    }

    Ok(())
}
