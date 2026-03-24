//! Vector benchmarks

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use vector::lexer::Lexer;
use vector::parser::Parser;
use vector::compiler::Compiler;
use vector::vm::VM;

const SIMPLE_PROGRAM: &str = r#"
let x = 42
let y = 3.14
let name = "hello"
fn add(a, b) {
    return a + b
}
if x > 0 {
    x
}
"#;

const ARITHMETIC_HEAVY: &str = r#"
let mut sum = 0
let mut i = 0
while i < 1000 {
    sum = sum + i * 2 - 1
    i = i + 1
}
sum
"#;

const FUNCTION_CALLS: &str = r#"
fn double(x) {
    return x * 2
}
let mut result = 1
let mut i = 0
while i < 100 {
    result = double(result)
    if result > 1000000 {
        result = 1
    }
    i = i + 1
}
result
"#;

const FIB_RECURSIVE: &str = r#"
fn fib(n) {
    if n < 2 {
        return n
    }
    return fib(n - 1) + fib(n - 2)
}
fib(15)
"#;

fn parse(source: &str) -> Vec<vector::parser::Stmt> {
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer);
    parser.parse().unwrap()
}

fn compile(source: &str) -> vector::compiler::Module {
    let ast = parse(source);
    let mut compiler = Compiler::new();
    compiler.compile(&ast).unwrap()
}

fn bench_lexer(c: &mut Criterion) {
    c.bench_function("lexer/simple", |b| {
        b.iter(|| {
            let mut lexer = Lexer::new(black_box(SIMPLE_PROGRAM));
            while let Ok(token) = lexer.next_token() {
                if token.kind == vector::lexer::TokenKind::Eof {
                    break;
                }
                black_box(token);
            }
        })
    });

    c.bench_function("lexer/arithmetic", |b| {
        b.iter(|| {
            let mut lexer = Lexer::new(black_box(ARITHMETIC_HEAVY));
            while let Ok(token) = lexer.next_token() {
                if token.kind == vector::lexer::TokenKind::Eof {
                    break;
                }
                black_box(token);
            }
        })
    });
}

fn bench_parser(c: &mut Criterion) {
    c.bench_function("parser/simple", |b| {
        b.iter(|| {
            let lexer = Lexer::new(black_box(SIMPLE_PROGRAM));
            let mut parser = Parser::new(lexer);
            black_box(parser.parse())
        })
    });

    c.bench_function("parser/arithmetic", |b| {
        b.iter(|| {
            let lexer = Lexer::new(black_box(ARITHMETIC_HEAVY));
            let mut parser = Parser::new(lexer);
            black_box(parser.parse())
        })
    });

    c.bench_function("parser/functions", |b| {
        b.iter(|| {
            let lexer = Lexer::new(black_box(FUNCTION_CALLS));
            let mut parser = Parser::new(lexer);
            black_box(parser.parse())
        })
    });
}

fn bench_compiler(c: &mut Criterion) {
    let simple_ast = parse(SIMPLE_PROGRAM);
    let arithmetic_ast = parse(ARITHMETIC_HEAVY);
    let functions_ast = parse(FUNCTION_CALLS);

    c.bench_function("compiler/simple", |b| {
        b.iter(|| {
            let mut compiler = Compiler::new();
            black_box(compiler.compile(black_box(&simple_ast)))
        })
    });

    c.bench_function("compiler/arithmetic", |b| {
        b.iter(|| {
            let mut compiler = Compiler::new();
            black_box(compiler.compile(black_box(&arithmetic_ast)))
        })
    });

    c.bench_function("compiler/functions", |b| {
        b.iter(|| {
            let mut compiler = Compiler::new();
            black_box(compiler.compile(black_box(&functions_ast)))
        })
    });
}

fn bench_vm_execution(c: &mut Criterion) {
    let arithmetic_module = compile(ARITHMETIC_HEAVY);
    let function_module = compile(FUNCTION_CALLS);
    let fib_module = compile(FIB_RECURSIVE);

    c.bench_function("vm/arithmetic_loop", |b| {
        b.iter(|| {
            let mut vm = VM::new_without_jit();
            black_box(vm.run(black_box(arithmetic_module.clone())))
        })
    });

    c.bench_function("vm/function_calls", |b| {
        b.iter(|| {
            let mut vm = VM::new_without_jit();
            black_box(vm.run(black_box(function_module.clone())))
        })
    });

    c.bench_function("vm/fib_15_recursive", |b| {
        b.iter(|| {
            let mut vm = VM::new_without_jit();
            black_box(vm.run(black_box(fib_module.clone())))
        })
    });
}

fn bench_end_to_end(c: &mut Criterion) {
    c.bench_function("e2e/arithmetic_loop", |b| {
        b.iter(|| {
            let module = compile(black_box(ARITHMETIC_HEAVY));
            let mut vm = VM::new_without_jit();
            black_box(vm.run(module))
        })
    });

    c.bench_function("e2e/fib_15", |b| {
        b.iter(|| {
            let module = compile(black_box(FIB_RECURSIVE));
            let mut vm = VM::new_without_jit();
            black_box(vm.run(module))
        })
    });
}

fn bench_fib_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("fib_scaling");
    
    for n in [5, 10, 15, 20] {
        let code = format!(r#"
fn fib(n) {{
    if n < 2 {{
        return n
    }}
    return fib(n - 1) + fib(n - 2)
}}
fib({})
"#, n);
        
        let module = compile(&code);
        
        group.bench_with_input(BenchmarkId::from_parameter(n), &module, |b, module| {
            b.iter(|| {
                let mut vm = VM::new_without_jit();
                black_box(vm.run(black_box(module.clone())))
            })
        });
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_lexer,
    bench_parser,
    bench_compiler,
    bench_vm_execution,
    bench_end_to_end,
    bench_fib_scaling,
);
criterion_main!(benches);
