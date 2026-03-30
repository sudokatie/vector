//! Performance benchmarks for Vector

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use vector::{Vector, lexer::Lexer, parser::Parser, compiler::Compiler};

/// Benchmark startup time
fn bench_startup(c: &mut Criterion) {
    c.bench_function("startup_repl", |b| {
        b.iter(|| {
            let vector = Vector::new();
            black_box(vector);
        });
    });

    c.bench_function("startup_no_jit", |b| {
        b.iter(|| {
            let vector = Vector::new_without_jit();
            black_box(vector);
        });
    });
}

/// Benchmark bytecode compilation speed
fn bench_compile(c: &mut Criterion) {
    let sources = vec![
        ("small", "let x = 1 + 2\nx * 3"),
        ("medium", r#"
            fn fib(n) {
                if n < 2 {
                    return n
                }
                return fib(n - 1) + fib(n - 2)
            }
            fib(10)
        "#),
        ("large", &generate_large_source(1000)),
    ];

    let mut group = c.benchmark_group("compile");
    for (name, source) in sources {
        group.bench_with_input(BenchmarkId::new("bytecode", name), source, |b, src| {
            b.iter(|| {
                let lexer = Lexer::new(src);
                let mut parser = Parser::new(lexer);
                let stmts = parser.parse().unwrap();
                let mut compiler = Compiler::new();
                let module = compiler.compile(&stmts).unwrap();
                black_box(module)
            });
        });
    }
    group.finish();
}

/// Benchmark interpreter speed
fn bench_interpreter(c: &mut Criterion) {
    let mut group = c.benchmark_group("interpreter");
    
    // Simple arithmetic
    group.bench_function("arithmetic_simple", |b| {
        let mut vector = Vector::new_without_jit();
        b.iter(|| {
            let result = vector.eval("1 + 2 * 3 - 4 / 2").unwrap();
            black_box(result)
        });
    });

    // Loop
    group.bench_function("loop_1000", |b| {
        let mut vector = Vector::new_without_jit();
        vector.eval("let sum = 0").unwrap();
        b.iter(|| {
            let result = vector.eval(r#"
                let i = 0
                while i < 1000 {
                    i = i + 1
                }
                i
            "#).unwrap();
            black_box(result)
        });
    });

    // Function calls
    group.bench_function("function_calls", |b| {
        let mut vector = Vector::new_without_jit();
        vector.eval("fn add(a, b) { return a + b }").unwrap();
        b.iter(|| {
            let result = vector.eval(r#"
                let sum = 0
                let i = 0
                while i < 100 {
                    sum = add(sum, i)
                    i = i + 1
                }
                sum
            "#).unwrap();
            black_box(result)
        });
    });

    // Array operations
    group.bench_function("array_ops", |b| {
        let mut vector = Vector::new_without_jit();
        b.iter(|| {
            let result = vector.eval(r#"
                let arr = []
                let i = 0
                while i < 100 {
                    push(arr, i)
                    i = i + 1
                }
                len(arr)
            "#).unwrap();
            black_box(result)
        });
    });

    // Fibonacci (recursive)
    group.bench_function("fib_20", |b| {
        let mut vector = Vector::new_without_jit();
        vector.eval(r#"
            fn fib(n) {
                if n < 2 {
                    return n
                }
                return fib(n - 1) + fib(n - 2)
            }
        "#).unwrap();
        b.iter(|| {
            let result = vector.eval("fib(20)").unwrap();
            black_box(result)
        });
    });

    group.finish();
}

/// Benchmark JIT compilation and execution
fn bench_jit(c: &mut Criterion) {
    let mut group = c.benchmark_group("jit");

    // Hot loop (should trigger JIT)
    group.bench_function("hot_loop", |b| {
        let mut vector = Vector::new();
        b.iter(|| {
            let result = vector.eval(r#"
                let sum = 0
                let i = 0
                while i < 10000 {
                    sum = sum + i
                    i = i + 1
                }
                sum
            "#).unwrap();
            black_box(result)
        });
    });

    group.finish();
}

/// Benchmark GC
fn bench_gc(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc");

    // Allocation stress
    group.bench_function("alloc_stress", |b| {
        let mut vector = Vector::new_without_jit();
        b.iter(|| {
            let result = vector.eval(r#"
                let arrays = []
                let i = 0
                while i < 100 {
                    push(arrays, [1, 2, 3, 4, 5])
                    i = i + 1
                }
                len(arrays)
            "#).unwrap();
            black_box(result)
        });
    });

    group.finish();
}

/// Benchmark lexer
fn bench_lexer(c: &mut Criterion) {
    let source = generate_large_source(1000);
    
    c.bench_function("lexer_1000_lines", |b| {
        b.iter(|| {
            let lexer = Lexer::new(&source);
            let tokens: Vec<_> = lexer.collect();
            black_box(tokens)
        });
    });
}

/// Benchmark parser
fn bench_parser(c: &mut Criterion) {
    let source = generate_large_source(1000);
    
    c.bench_function("parser_1000_lines", |b| {
        b.iter(|| {
            let lexer = Lexer::new(&source);
            let mut parser = Parser::new(lexer);
            let stmts = parser.parse().unwrap();
            black_box(stmts)
        });
    });
}

/// Benchmark string interning
fn bench_intern(c: &mut Criterion) {
    use vector::runtime::StringInterner;
    
    let mut group = c.benchmark_group("intern");
    
    group.bench_function("intern_new", |b| {
        let mut interner = StringInterner::new();
        let mut i = 0;
        b.iter(|| {
            let s = format!("string_{}", i);
            i += 1;
            let interned = interner.intern(&s);
            black_box(interned)
        });
    });

    group.bench_function("intern_cached", |b| {
        let mut interner = StringInterner::new();
        let _ = interner.intern("cached_string");
        b.iter(|| {
            let interned = interner.intern("cached_string");
            black_box(interned)
        });
    });

    group.finish();
}

/// Benchmark inline cache
fn bench_inline_cache(c: &mut Criterion) {
    use vector::jit::{InlineCacheManager, AccessSite, ShapeId, PropertySlot};
    
    let mut group = c.benchmark_group("inline_cache");
    
    group.bench_function("lookup_hit", |b| {
        let mut icm = InlineCacheManager::new();
        let site = AccessSite::new(0, "property");
        let shape = icm.new_shape_id();
        let slot = PropertySlot { offset: 0, is_own: true };
        icm.record(site, shape, slot);
        
        b.iter(|| {
            let result = icm.lookup(site, shape);
            black_box(result)
        });
    });

    group.bench_function("lookup_miss", |b| {
        let mut icm = InlineCacheManager::new();
        let site = AccessSite::new(0, "property");
        let shape = icm.new_shape_id();
        
        b.iter(|| {
            let result = icm.lookup(site, shape);
            black_box(result)
        });
    });

    group.finish();
}

/// Generate a large source file for benchmarking
fn generate_large_source(lines: usize) -> String {
    let mut source = String::with_capacity(lines * 50);
    
    for i in 0..lines {
        source.push_str(&format!("let x_{} = {} + {} * {}\n", i, i, i + 1, i + 2));
    }
    
    source.push_str("x_0");
    source
}

criterion_group!(
    benches,
    bench_startup,
    bench_compile,
    bench_interpreter,
    bench_jit,
    bench_gc,
    bench_lexer,
    bench_parser,
    bench_intern,
    bench_inline_cache,
);

criterion_main!(benches);
