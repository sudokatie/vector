//! Vector CLI and REPL

use vector::Vector;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::env;
use std::fs;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    match args.len() {
        1 => repl(),
        2 => {
            if args[1] == "--help" || args[1] == "-h" {
                print_help();
            } else if args[1] == "--version" || args[1] == "-v" {
                println!("vector 0.1.0");
            } else {
                run_file(&args[1]);
            }
        }
        3 => {
            if args[1] == "-c" || args[1] == "--code" {
                run_code(&args[2]);
            } else if args[1] == "--disasm" || args[1] == "-d" {
                disassemble_file(&args[2]);
            } else {
                eprintln!("Unknown option: {}", args[1]);
                process::exit(1);
            }
        }
        _ => {
            print_help();
            process::exit(1);
        }
    }
}

fn print_help() {
    println!("vector - JIT-compiled scripting language");
    println!();
    println!("USAGE:");
    println!("    vector                  Start REPL");
    println!("    vector <file>           Run script file");
    println!("    vector -c <code>        Run code string");
    println!("    vector --disasm <file>  Show bytecode disassembly");
    println!();
    println!("OPTIONS:");
    println!("    -h, --help      Print help");
    println!("    -v, --version   Print version");
    println!("    -c, --code      Run code from argument");
    println!("    -d, --disasm    Disassemble file");
}

fn repl() {
    println!("Vector 0.1.0 - Type 'exit' or Ctrl+D to quit");

    let mut rl = match DefaultEditor::new() {
        Ok(editor) => editor,
        Err(e) => {
            eprintln!("Failed to initialize readline: {}", e);
            simple_repl();
            return;
        }
    };

    let mut vector = Vector::new();

    loop {
        match rl.readline(">> ") {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if line == "exit" || line == "quit" {
                    break;
                }

                rl.add_history_entry(line).ok();

                match vector.eval(line) {
                    Ok(value) => {
                        if !matches!(value, vector::vm::Value::Nil) {
                            println!("{}", value);
                        }
                    }
                    Err(e) => eprintln!("Error: {}", e),
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
            }
            Err(ReadlineError::Eof) => {
                println!();
                break;
            }
            Err(e) => {
                eprintln!("Readline error: {}", e);
                break;
            }
        }
    }
}

fn simple_repl() {
    use std::io::{self, BufRead, Write};

    let mut vector = Vector::new();
    let stdin = io::stdin();

    loop {
        print!(">> ");
        io::stdout().flush().unwrap();

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if line == "exit" || line == "quit" {
                    break;
                }

                match vector.eval(line) {
                    Ok(value) => {
                        if !matches!(value, vector::vm::Value::Nil) {
                            println!("{}", value);
                        }
                    }
                    Err(e) => eprintln!("Error: {}", e),
                }
            }
            Err(e) => {
                eprintln!("Read error: {}", e);
                break;
            }
        }
    }
}

fn run_file(path: &str) {
    let source = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", path, e);
            process::exit(1);
        }
    };

    let mut vector = Vector::new();
    match vector.eval(&source) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    }
}

fn run_code(code: &str) {
    let mut vector = Vector::new();
    match vector.eval(code) {
        Ok(value) => {
            if !matches!(value, vector::vm::Value::Nil) {
                println!("{}", value);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    }
}

fn disassemble_file(path: &str) {
    let source = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", path, e);
            process::exit(1);
        }
    };

    use vector::lexer::Lexer;
    use vector::parser::Parser;
    use vector::compiler::Compiler;

    let lexer = Lexer::new(&source);
    let mut parser = Parser::new(lexer);

    let stmts = match parser.parse() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            process::exit(1);
        }
    };

    let mut compiler = Compiler::new();
    let module = match compiler.compile(&stmts) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Compile error: {}", e);
            process::exit(1);
        }
    };

    println!("{}", module.main.chunk.disassemble(&format!("main ({})", path)));

    for (i, func) in module.functions.iter().enumerate() {
        println!();
        println!("{}", func.chunk.disassemble(&format!("function_{} ({})", i, func.name)));
    }
}
