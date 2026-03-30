//! Vector CLI and REPL

use vector::Vector;
use vector::vm::VM;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::env;
use std::fs;
use std::io::Write;
use std::process;

/// CLI options
struct Options {
    show_stats: bool,
    show_gc_stats: bool,
    no_jit: bool,
    heap_size: Option<usize>,
    opt_level: u8,
    compile_output: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            show_stats: false,
            show_gc_stats: false,
            no_jit: false,
            heap_size: None,
            opt_level: 1, // Default optimization level
            compile_output: None,
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut opts = Options::default();
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
            "--stats" | "-s" => opts.show_stats = true,
            "--gc-stats" => opts.show_gc_stats = true,
            "--no-jit" => opts.no_jit = true,
            "-O0" => opts.opt_level = 0,
            "-O1" => opts.opt_level = 1,
            "-O2" => opts.opt_level = 2,
            "--heap-size" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --heap-size requires an argument");
                    process::exit(1);
                }
                opts.heap_size = Some(parse_size(&args[i]));
            }
            "--compile" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --compile requires an output file");
                    process::exit(1);
                }
                opts.compile_output = Some(args[i].clone());
            }
            "-c" | "--code" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: -c requires an argument");
                    process::exit(1);
                }
                run_code(&args[i], &opts);
                return;
            }
            "-d" | "--disasm" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --disasm requires a file");
                    process::exit(1);
                }
                disassemble_file(&args[i]);
                return;
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
        repl(&opts);
    } else if opts.compile_output.is_some() {
        compile_file(positional[0], &opts);
    } else {
        run_file(positional[0], &opts);
    }
}

/// Parse size string (e.g., "16M", "1G", "1024K")
fn parse_size(s: &str) -> usize {
    let s = s.trim();
    let (num, mult) = if s.ends_with('G') || s.ends_with('g') {
        (&s[..s.len()-1], 1024 * 1024 * 1024)
    } else if s.ends_with('M') || s.ends_with('m') {
        (&s[..s.len()-1], 1024 * 1024)
    } else if s.ends_with('K') || s.ends_with('k') {
        (&s[..s.len()-1], 1024)
    } else {
        (s, 1)
    };
    
    num.parse::<usize>().unwrap_or(64 * 1024 * 1024) * mult
}

fn print_help() {
    println!("vector - JIT-compiled scripting language");
    println!();
    println!("USAGE:");
    println!("    vector                      Start REPL");
    println!("    vector <file>               Run script file");
    println!("    vector -c <code>            Run code string");
    println!("    vector --compile <out> <in> Compile to bytecode");
    println!("    vector --disasm <file>      Show bytecode disassembly");
    println!();
    println!("OPTIONS:");
    println!("    -h, --help          Print help");
    println!("    -v, --version       Print version");
    println!("    -c, --code          Run code from argument");
    println!("    -d, --disasm        Disassemble file");
    println!("    -s, --stats         Show execution statistics");
    println!("    --gc-stats          Show GC statistics");
    println!("    --no-jit            Disable JIT compilation");
    println!("    --heap-size SIZE    Set max heap size (e.g., 64M, 1G)");
    println!("    --compile FILE      Compile to bytecode file");
    println!("    -O0                 No optimization");
    println!("    -O1                 Basic optimization (default)");
    println!("    -O2                 Full optimization");
}

fn create_vector(opts: &Options) -> Vector {
    if opts.no_jit {
        if let Some(size) = opts.heap_size {
            Vector::with_heap_size_no_jit(size)
        } else {
            Vector::new_without_jit()
        }
    } else if let Some(size) = opts.heap_size {
        Vector::with_heap_size(size)
    } else {
        Vector::new()
    }
}

fn repl(opts: &Options) {
    println!("Vector 0.1.0 - Type 'exit' or Ctrl+D to quit");
    if opts.no_jit {
        println!("(JIT disabled)");
    }
    if opts.opt_level == 0 {
        println!("(Optimization: none)");
    } else if opts.opt_level == 2 {
        println!("(Optimization: full)");
    }

    let mut rl = match DefaultEditor::new() {
        Ok(editor) => editor,
        Err(e) => {
            eprintln!("Failed to initialize readline: {}", e);
            simple_repl(opts);
            return;
        }
    };

    let mut vector = create_vector(opts);

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

    if opts.show_gc_stats {
        print_gc_stats(&vector);
    }
}

fn simple_repl(opts: &Options) {
    use std::io::{self, BufRead};

    let mut vector = create_vector(opts);
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

fn run_file(path: &str, opts: &Options) {
    let source = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading file '{}': {}", path, e);
            process::exit(1);
        }
    };

    let mut vector = create_vector(opts);

    match vector.eval(&source) {
        Ok(_) => {
            if opts.show_stats {
                print_stats(&vector);
            }
            if opts.show_gc_stats {
                print_gc_stats(&vector);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    }
}

fn run_code(code: &str, opts: &Options) {
    let mut vector = create_vector(opts);

    match vector.eval(code) {
        Ok(value) => {
            if !matches!(value, vector::vm::Value::Nil) {
                println!("{}", value);
            }
            if opts.show_stats {
                print_stats(&vector);
            }
            if opts.show_gc_stats {
                print_gc_stats(&vector);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    }
}

fn compile_file(path: &str, opts: &Options) {
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

    // Serialize module to bytecode
    let bytecode = module.to_bytes();
    
    let output_path = opts.compile_output.as_ref().unwrap();
    match fs::write(output_path, &bytecode) {
        Ok(_) => {
            println!("Compiled {} -> {} ({} bytes)", path, output_path, bytecode.len());
        }
        Err(e) => {
            eprintln!("Error writing file '{}': {}", output_path, e);
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

fn print_gc_stats(vector: &Vector) {
    eprintln!();
    eprintln!("=== GC Statistics ===");
    
    let stats = vector.gc_stats();
    eprintln!("  Collections: {}", stats.collections);
    eprintln!("  Bytes allocated: {}", stats.bytes_allocated);
    eprintln!("  Bytes freed: {}", stats.bytes_freed);
    eprintln!("  Live objects: {}", stats.live_objects);
    eprintln!("  Last collection:");
    eprintln!("    Objects freed: {}", stats.last_freed_objects);
    eprintln!("    Bytes freed: {}", stats.last_freed_bytes);
    eprintln!("    Time: {}µs", stats.last_collect_us);
    
    let (allocated, max_size, threshold) = vector.heap_info();
    eprintln!("  Heap allocated: {} bytes", allocated);
    eprintln!("  Heap max size: {} bytes", max_size);
    eprintln!("  GC threshold: {} bytes", threshold);
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
