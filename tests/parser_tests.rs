//! Parser integration tests

use vector::lexer::Lexer;
use vector::parser::{Parser, Expr, Stmt, BinaryOp, UnaryOp, Pattern};

fn parse(source: &str) -> Vec<Stmt> {
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer);
    parser.parse().unwrap()
}

fn parse_expr(source: &str) -> Expr {
    let stmts = parse(source);
    match &stmts[0] {
        Stmt::Expr(e) => e.clone(),
        _ => panic!("Expected expression statement"),
    }
}

#[test]
fn test_parse_all_literals() {
    assert!(matches!(parse_expr("nil"), Expr::Nil));
    assert!(matches!(parse_expr("true"), Expr::Bool(true)));
    assert!(matches!(parse_expr("false"), Expr::Bool(false)));
    assert!(matches!(parse_expr("42"), Expr::Int(42)));
    assert!(matches!(parse_expr("3.14"), Expr::Float(f) if (f - 3.14).abs() < 0.001));
    assert!(matches!(parse_expr("\"hello\""), Expr::String(s) if s == "hello"));
}

#[test]
fn test_parse_binary_operators() {
    let ops = vec![
        ("1 + 2", BinaryOp::Add),
        ("1 - 2", BinaryOp::Sub),
        ("1 * 2", BinaryOp::Mul),
        ("1 / 2", BinaryOp::Div),
        ("1 % 2", BinaryOp::Mod),
        ("2 ** 3", BinaryOp::Pow),
        ("1 == 2", BinaryOp::Eq),
        ("1 != 2", BinaryOp::Ne),
        ("1 < 2", BinaryOp::Lt),
        ("1 <= 2", BinaryOp::Le),
        ("1 > 2", BinaryOp::Gt),
        ("1 >= 2", BinaryOp::Ge),
        ("true and false", BinaryOp::And),
        ("true or false", BinaryOp::Or),
        ("1 & 2", BinaryOp::BitAnd),
        ("1 | 2", BinaryOp::BitOr),
        ("1 ^ 2", BinaryOp::BitXor),
        ("1 << 2", BinaryOp::Shl),
        ("1 >> 2", BinaryOp::Shr),
        ("\"a\" ++ \"b\"", BinaryOp::Concat),
        ("0..10", BinaryOp::Range),
        ("0..=10", BinaryOp::RangeInclusive),
    ];
    
    for (source, expected_op) in ops {
        let expr = parse_expr(source);
        match expr {
            Expr::Binary(_, op, _) => assert_eq!(op, expected_op, "Failed for: {}", source),
            _ => panic!("Expected binary expression for: {}", source),
        }
    }
}

#[test]
fn test_parse_unary_operators() {
    let ops = vec![
        ("-42", UnaryOp::Neg),
        ("not true", UnaryOp::Not),
        ("~0", UnaryOp::BitNot),
    ];
    
    for (source, expected_op) in ops {
        let expr = parse_expr(source);
        match expr {
            Expr::Unary(op, _) => assert_eq!(op, expected_op, "Failed for: {}", source),
            _ => panic!("Expected unary expression for: {}", source),
        }
    }
}

#[test]
fn test_parse_precedence() {
    // Multiplication before addition
    let expr = parse_expr("1 + 2 * 3");
    match expr {
        Expr::Binary(_, BinaryOp::Add, right) => {
            assert!(matches!(*right, Expr::Binary(_, BinaryOp::Mul, _)));
        }
        _ => panic!("Wrong precedence"),
    }
    
    // Power is right-associative
    let expr = parse_expr("2 ** 3 ** 4");
    match expr {
        Expr::Binary(_, BinaryOp::Pow, right) => {
            assert!(matches!(*right, Expr::Binary(_, BinaryOp::Pow, _)));
        }
        _ => panic!("Power should be right-associative"),
    }
}

#[test]
fn test_parse_function_call() {
    let expr = parse_expr("foo(1, 2, 3)");
    match expr {
        Expr::Call(callee, args) => {
            assert!(matches!(*callee, Expr::Identifier(s) if s == "foo"));
            assert_eq!(args.len(), 3);
        }
        _ => panic!("Expected function call"),
    }
}

#[test]
fn test_parse_array_literal() {
    let expr = parse_expr("[1, 2, 3]");
    match expr {
        Expr::Array(elements) => {
            assert_eq!(elements.len(), 3);
        }
        _ => panic!("Expected array"),
    }
}

#[test]
fn test_parse_table_literal() {
    let expr = parse_expr("{ name: \"Alice\", age: 30 }");
    match expr {
        Expr::Table(pairs) => {
            assert_eq!(pairs.len(), 2);
        }
        _ => panic!("Expected table"),
    }
}

#[test]
fn test_parse_index_and_field() {
    let expr = parse_expr("arr[0]");
    assert!(matches!(expr, Expr::Index(_, _)));
    
    let expr = parse_expr("obj.field");
    assert!(matches!(expr, Expr::Field(_, _)));
}

#[test]
fn test_parse_anonymous_function() {
    let expr = parse_expr("fn(x) { x * 2 }");
    match expr {
        Expr::Function(def) => {
            assert_eq!(def.name, None);
            assert_eq!(def.params.len(), 1);
        }
        _ => panic!("Expected function"),
    }
}

#[test]
fn test_parse_let_statement() {
    let stmts = parse("let x = 42");
    match &stmts[0] {
        Stmt::Let(name, is_mut, _) => {
            assert_eq!(name, "x");
            assert!(!is_mut);
        }
        _ => panic!("Expected let"),
    }
    
    let stmts = parse("let mut y = 0");
    match &stmts[0] {
        Stmt::Let(name, is_mut, _) => {
            assert_eq!(name, "y");
            assert!(is_mut);
        }
        _ => panic!("Expected let mut"),
    }
}

#[test]
fn test_parse_if_statement() {
    let stmts = parse("if x > 0 { 1 } else { 2 }");
    match &stmts[0] {
        Stmt::Expr(Expr::If(_cond, _then_branch, else_branch)) => {
            // then_branch is always present (Box<Expr>)
            // else_branch is Option<Box<Expr>>
            assert!(else_branch.is_some());
        }
        _ => panic!("Expected if expression in Stmt::Expr"),
    }
}

#[test]
fn test_parse_while_statement() {
    let stmts = parse("while x > 0 { x = x - 1 }");
    match &stmts[0] {
        Stmt::While(_, body) => {
            assert!(!body.is_empty());
        }
        _ => panic!("Expected while"),
    }
}

#[test]
fn test_parse_for_statement() {
    let stmts = parse("for i in 0..10 { print(i) }");
    match &stmts[0] {
        Stmt::For(name, _, body) => {
            assert_eq!(name, "i");
            assert!(!body.is_empty());
        }
        _ => panic!("Expected for"),
    }
}

#[test]
fn test_parse_function_definition() {
    let stmts = parse("fn add(a, b) { return a + b }");
    match &stmts[0] {
        Stmt::Function(def) => {
            assert_eq!(def.name, Some("add".to_string()));
            assert_eq!(def.params.len(), 2);
        }
        _ => panic!("Expected function"),
    }
}

#[test]
fn test_parse_match_expression() {
    let stmts = parse("match x { 0 => \"zero\", _ => \"other\" }");
    match &stmts[0] {
        Stmt::Expr(Expr::Match(_, arms)) => {
            assert_eq!(arms.len(), 2);
            assert!(matches!(&arms[0].pattern, Pattern::Literal(_)));
            assert!(matches!(&arms[1].pattern, Pattern::Wildcard));
        }
        _ => panic!("Expected match expression"),
    }
}

#[test]
fn test_parse_try_expression() {
    let expr = parse_expr("try risky()");
    assert!(matches!(expr, Expr::Try(_)));
}

#[test]
fn test_parse_string_interpolation() {
    let expr = parse_expr("\"Hello, {name}!\"");
    assert!(matches!(expr, Expr::Interpolation(_)));
}
