use crate::frontend::lexer::Lexer;
use crate::frontend::parser::Parser;
use crate::frontend::compiler::Compiler;
use crate::vm::VM;

use std::io::{self, Write, BufRead};

pub fn run_repl() {
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
    let _should_print = matches!(ast.last(), Some(crate::frontend::ast::Stmt::Expression(_)));

    let compiler = Compiler::new("repl", false, false, None);
    let function = compiler.compile(&ast)?;

    vm.interpret(function)?;

    Ok(())
}
