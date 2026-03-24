//! Vector benchmarks

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use vector::lexer::Lexer;

fn bench_lexer(c: &mut Criterion) {
    let source = r#"
        let x = 42
        let y = 3.14
        let name = "hello"
        fn add(a, b) {
            return a + b
        }
        if x > 0 {
            print(x)
        }
    "#;

    c.bench_function("lexer", |b| {
        b.iter(|| {
            let mut lexer = Lexer::new(black_box(source));
            while let Ok(token) = lexer.next_token() {
                if token.kind == vector::lexer::TokenKind::Eof {
                    break;
                }
                black_box(token);
            }
        })
    });
}

criterion_group!(benches, bench_lexer);
criterion_main!(benches);
