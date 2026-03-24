//! Vector CLI and REPL

use vector::Vector;
use vector::vm::VM;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::env;
use std::fs;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    // Parse options
    let mut show_stats = false;
    let mut no_jit = false;
    let mut positional: Vec<&str> = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                print_help();
                return;
            }
            "--version" | "-v" => {
                println!("vector 0.1.0");
                return;
            }
            "--stats" | "-s" => show_stats = true,
            "--no-jit" => no_jit = true,
            "-c" | "--code" => {
                if i + 1 < args.len() {
                    run_code(&args[i + 1], show_stats, no_jit);
                    return;
                } else {
                    eprintln!("Error: -c requires an argument");
                    process::exit(1);
                }
            }
            "-d" | "--disasm" => {
                if i + 1 < args.len() {
                    disassemble_file(&args[i + 1]);
                    return;
                } else {
                    eprintln!("Error: --disasm requires a file");
                    process::exit(1);
                }
            }
            arg if arg.starts_with('-') => {
                eprintln!("Unknown option: {}", arg);
                process::exit(1);
            }
            arg => positional.push(arg),
        }
        i += 1;
    }

    if positional.is_empty() {
        repl(no_jit);
    } else {
        run_file(positional[0], show_stats, no_jit);
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
    println!("    -s, --stats     Show execution statistics");
    println!("    --no-jit        Disable JIT compilation");
}

fn repl(no_jit: bool) {
    println!("Vector 0.1.0 - Type 'exit' or Ctrl+D to quit");
    if no_jit {
        println!("(JIT disabled)");
    }

    let mut rl = match DefaultEditor::new() {
        Ok(editor) => editor,
        Err(e) => {
            eprintln!("Failed to initialize readline: {}", e);
            simple_repl(no_jit);
            return;
        }
    };

    let mut vector = if no_jit {
        Vector::new_without_jit()
    } else {
        Vector::new()
    };

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

fn simple_repl(no_jit: bool) {
    use std::io::{self, BufRead, Write};

    let mut vector = if no_jit {
        Vector::new_without_jit()
    } else {
        Vector::new()
    };
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

fn run_file(path: &str, show_stats: bool, no_jit: bool) {
    let source = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", path, e);
            process::exit(1);
        }
    };

    let mut vector = if no_jit {
        Vector::new_without_jit()
    } else {
        Vector::new()
    };

    match vector.eval(&source) {
        Ok(_) => {
            if show_stats {
                print_stats(&vector);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    }
}

fn run_code(code: &str, show_stats: bool, no_jit: bool) {
    let mut vector = if no_jit {
        Vector::new_without_jit()
    } else {
        Vector::new()
    };

    match vector.eval(code) {
        Ok(value) => {
            if !matches!(value, vector::vm::Value::Nil) {
                println!("{}", value);
            }
            if show_stats {
                print_stats(&vector);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    }
}

fn print_stats(vector: &Vector) {
    eprintln!();
    eprintln!("=== Execution Statistics ===");

    if let Some(profiler_stats) = vector.profiler_stats() {
        eprintln!("Profiler:");
        eprintln!("  Total function calls: {}", profiler_stats.total_calls);
        eprintln!("  Total loop iterations: {}", profiler_stats.total_loop_iterations);
        eprintln!("  Functions profiled: {}", profiler_stats.functions_profiled);
        eprintln!("  Functions compiled: {}", profiler_stats.functions_compiled);
    }

    if let Some(jit_stats) = vector.jit_stats() {
        eprintln!("JIT:");
        eprintln!("  Functions compiled: {}", jit_stats.functions_compiled);
        eprintln!("  Compilation time: {}µs", jit_stats.compilation_time_us);
        eprintln!("  Native calls: {}", jit_stats.native_calls);
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
