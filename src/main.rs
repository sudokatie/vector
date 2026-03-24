//! Vector CLI

use vector::Vector;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        // REPL mode
        repl();
    } else {
        // Run file
        run_file(&args[1]);
    }
}

fn repl() {
    println!("Vector 0.1.0");
    println!("Type expressions to evaluate. Ctrl+D to exit.");

    let mut vector = Vector::new();

    loop {
        print!("> ");
        use std::io::Write;
        std::io::stdout().flush().unwrap();

        let mut line = String::new();
        match std::io::stdin().read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                match vector.eval(line) {
                    Ok(value) => println!("{}", value),
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
    let mut vector = Vector::new();

    match vector.run_file(path) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
