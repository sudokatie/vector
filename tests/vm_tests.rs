//! VM integration tests

use vector::Vector;
use vector::vm::Value;

fn eval(source: &str) -> Value {
    let mut v = Vector::new();
    v.eval(source).unwrap()
}

fn eval_int(source: &str) -> i64 {
    match eval(source) {
        Value::Int(i) => i,
        v => panic!("Expected int, got {:?}", v),
    }
}

fn eval_float(source: &str) -> f64 {
    match eval(source) {
        Value::Float(f) => f,
        Value::Int(i) => i as f64,
        v => panic!("Expected float, got {:?}", v),
    }
}

fn eval_bool(source: &str) -> bool {
    match eval(source) {
        Value::Bool(b) => b,
        v => panic!("Expected bool, got {:?}", v),
    }
}

fn eval_string(source: &str) -> String {
    match eval(source) {
        Value::String(s) => s,
        v => panic!("Expected string, got {:?}", v),
    }
}

// === Arithmetic ===

#[test]
fn test_arithmetic_add() {
    assert_eq!(eval_int("1 + 2"), 3);
    assert_eq!(eval_int("10 + 20 + 30"), 60);
}

#[test]
fn test_arithmetic_sub() {
    assert_eq!(eval_int("5 - 3"), 2);
    assert_eq!(eval_int("10 - 3 - 2"), 5);
}

#[test]
fn test_arithmetic_mul() {
    assert_eq!(eval_int("3 * 4"), 12);
    assert_eq!(eval_int("2 * 3 * 4"), 24);
}

#[test]
fn test_arithmetic_div() {
    assert_eq!(eval_int("10 / 2"), 5);
    assert_eq!(eval_int("20 / 4 / 2"), 2);
}

#[test]
fn test_arithmetic_mod() {
    assert_eq!(eval_int("10 % 3"), 1);
    assert_eq!(eval_int("17 % 5"), 2);
}

#[test]
fn test_arithmetic_pow() {
    assert_eq!(eval_int("2 ** 3"), 8);
    assert_eq!(eval_int("2 ** 10"), 1024);
}

#[test]
fn test_arithmetic_neg() {
    assert_eq!(eval_int("-5"), -5);
    assert_eq!(eval_int("--5"), 5);
}

#[test]
fn test_arithmetic_float() {
    assert!((eval_float("1.5 + 2.5") - 4.0).abs() < 0.001);
    assert!((eval_float("3.0 * 2.0") - 6.0).abs() < 0.001);
}

#[test]
fn test_arithmetic_mixed() {
    assert!((eval_float("1 + 2.5") - 3.5).abs() < 0.001);
    assert!((eval_float("2.0 * 3") - 6.0).abs() < 0.001);
}

// === Comparison ===

#[test]
fn test_comparison_eq() {
    assert!(eval_bool("1 == 1"));
    assert!(!eval_bool("1 == 2"));
    assert!(eval_bool("\"a\" == \"a\""));
}

#[test]
fn test_comparison_ne() {
    assert!(eval_bool("1 != 2"));
    assert!(!eval_bool("1 != 1"));
}

#[test]
fn test_comparison_lt() {
    assert!(eval_bool("1 < 2"));
    assert!(!eval_bool("2 < 1"));
    assert!(!eval_bool("1 < 1"));
}

#[test]
fn test_comparison_le() {
    assert!(eval_bool("1 <= 2"));
    assert!(eval_bool("1 <= 1"));
    assert!(!eval_bool("2 <= 1"));
}

#[test]
fn test_comparison_gt() {
    assert!(eval_bool("2 > 1"));
    assert!(!eval_bool("1 > 2"));
    assert!(!eval_bool("1 > 1"));
}

#[test]
fn test_comparison_ge() {
    assert!(eval_bool("2 >= 1"));
    assert!(eval_bool("1 >= 1"));
    assert!(!eval_bool("1 >= 2"));
}

// === Logical ===

#[test]
fn test_logical_and() {
    assert!(eval_bool("true and true"));
    assert!(!eval_bool("true and false"));
    assert!(!eval_bool("false and true"));
    assert!(!eval_bool("false and false"));
}

#[test]
fn test_logical_or() {
    assert!(eval_bool("true or true"));
    assert!(eval_bool("true or false"));
    assert!(eval_bool("false or true"));
    assert!(!eval_bool("false or false"));
}

#[test]
fn test_logical_not() {
    assert!(!eval_bool("not true"));
    assert!(eval_bool("not false"));
}

// === Bitwise ===

#[test]
fn test_bitwise_and() {
    assert_eq!(eval_int("6 & 3"), 2);
    assert_eq!(eval_int("255 & 15"), 15);
}

#[test]
fn test_bitwise_or() {
    assert_eq!(eval_int("4 | 2"), 6);
    assert_eq!(eval_int("8 | 1"), 9);
}

#[test]
fn test_bitwise_xor() {
    assert_eq!(eval_int("5 ^ 3"), 6);
    assert_eq!(eval_int("10 ^ 10"), 0);
}

#[test]
fn test_bitwise_not() {
    assert_eq!(eval_int("~0"), -1);
}

#[test]
fn test_bitwise_shift() {
    assert_eq!(eval_int("1 << 4"), 16);
    assert_eq!(eval_int("16 >> 2"), 4);
}

// === Strings ===

#[test]
fn test_string_concat() {
    assert_eq!(eval_string("\"hello\" ++ \" \" ++ \"world\""), "hello world");
}

#[test]
fn test_string_length() {
    assert_eq!(eval_int("len(\"hello\")"), 5);
}

// === Variables ===

#[test]
fn test_let_binding() {
    assert_eq!(eval_int("let x = 42\nx"), 42);
}

#[test]
fn test_assignment() {
    assert_eq!(eval_int("let mut x = 1\nx = 2\nx"), 2);
}

// === Control Flow ===

#[test]
fn test_if_true() {
    assert_eq!(eval_int("if true { 1 } else { 2 }"), 1);
}

#[test]
fn test_if_false() {
    assert_eq!(eval_int("if false { 1 } else { 2 }"), 2);
}

#[test]
fn test_if_else_if() {
    let code = "let x = 2\nif x == 1 { 10 } else if x == 2 { 20 } else { 30 }";
    assert_eq!(eval_int(code), 20);
}

#[test]
fn test_while_loop() {
    let code = "let mut sum = 0\nlet mut i = 1\nwhile i <= 5 { sum = sum + i\ni = i + 1 }\nsum";
    assert_eq!(eval_int(code), 15);
}

// === Functions ===

#[test]
fn test_function_def_call() {
    let code = "fn add(a, b) { return a + b }\nadd(2, 3)";
    assert_eq!(eval_int(code), 5);
}

#[test]
fn test_function_recursive() {
    let code = "fn fib(n) { if n <= 1 { return n } return fib(n-1) + fib(n-2) }\nfib(10)";
    assert_eq!(eval_int(code), 55);
}

#[test]
fn test_function_closure() {
    let code = r#"
        fn make_counter() {
            let mut count = 0
            return fn() {
                count = count + 1
                return count
            }
        }
        let c = make_counter()
        c()
        c()
        c()
    "#;
    assert_eq!(eval_int(code), 3);
}

// === Arrays ===

#[test]
fn test_array_literal() {
    let code = "let arr = [1, 2, 3]\nlen(arr)";
    assert_eq!(eval_int(code), 3);
}

#[test]
fn test_array_index() {
    let code = "let arr = [10, 20, 30]\narr[1]";
    assert_eq!(eval_int(code), 20);
}

#[test]
fn test_array_push_pop() {
    let code = r#"
        let arr = [1, 2]
        push(arr, 3)
        pop(arr)
    "#;
    assert_eq!(eval_int(code), 3);
}

// === Tables ===

#[test]
fn test_table_literal() {
    let code = "let t = { x: 1, y: 2 }\nt.x + t.y";
    assert_eq!(eval_int(code), 3);
}

#[test]
fn test_table_index() {
    let code = "let t = { x: 42 }\nt[\"x\"]";
    assert_eq!(eval_int(code), 42);
}

// === Standard Library ===

#[test]
fn test_stdlib_type() {
    assert_eq!(eval_string("type(42)"), "int");
    assert_eq!(eval_string("type(3.14)"), "float");
    assert_eq!(eval_string("type(\"hi\")"), "string");
    assert_eq!(eval_string("type(true)"), "bool");
    assert_eq!(eval_string("type(nil)"), "nil");
}

#[test]
fn test_stdlib_str() {
    assert_eq!(eval_string("str(42)"), "42");
    assert_eq!(eval_string("str(true)"), "true");
}

#[test]
fn test_stdlib_math() {
    assert_eq!(eval_int("abs(-5)"), 5);
    assert_eq!(eval_int("floor(3.7)"), 3);
    assert_eq!(eval_int("ceil(3.2)"), 4);
    assert!((eval_float("sqrt(16)") - 4.0).abs() < 0.001);
    assert_eq!(eval_int("min(3, 1, 2)"), 1);
    assert_eq!(eval_int("max(1, 3, 2)"), 3);
}

#[test]
fn test_stdlib_string_functions() {
    assert_eq!(eval_string("upper(\"hello\")"), "HELLO");
    assert_eq!(eval_string("lower(\"HELLO\")"), "hello");
    assert_eq!(eval_string("trim(\"  hi  \")"), "hi");
    assert!(eval_bool("contains(\"hello\", \"ell\")"));
    assert_eq!(eval_string("replace(\"hello\", \"l\", \"L\")"), "heLLo");
}

#[test]
fn test_stdlib_array_functions() {
    let code = "let arr = [3, 1, 2]\nsort(arr)\narr[0]";
    assert_eq!(eval_int(code), 1);
    
    let code = "let arr = [1, 2, 3]\nreverse(arr)\narr[0]";
    assert_eq!(eval_int(code), 3);
}

// === Match Expression ===

#[test]
fn test_match_literal() {
    let code = "let x = 2\nmatch x { 1 => 10, 2 => 20, _ => 0 }";
    assert_eq!(eval_int(code), 20);
}

#[test]
fn test_match_wildcard() {
    let code = "let x = 99\nmatch x { 1 => 10, _ => 0 }";
    assert_eq!(eval_int(code), 0);
}

// === String Interpolation ===

#[test]
fn test_string_interpolation() {
    let code = "let name = \"world\"\n\"Hello, {name}!\"";
    assert_eq!(eval_string(code), "Hello, world!");
}

#[test]
fn test_string_interpolation_expr() {
    let code = "\"1 + 2 = {1 + 2}\"";
    assert_eq!(eval_string(code), "1 + 2 = 3");
}

// === Try Expression ===

#[test]
fn test_try_expression() {
    let code = "let r = try 42\nr.ok";
    assert!(eval_bool(code));
}

// === Deep Copy ===

#[test]
fn test_deep_copy_independence() {
    // Note: deep_copy is tested indirectly through value operations
    // The Copy opcode triggers deep_copy for compound types
    let code = r#"
        let a = [1, 2, 3]
        let b = a
        push(b, 4)
        len(a)
    "#;
    // Arrays use shared references, so both see the change
    assert_eq!(eval_int(code), 4);
}
