//! End-to-end integration tests

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

fn eval_string(source: &str) -> String {
    match eval(source) {
        Value::String(s) => s,
        v => panic!("Expected string, got {:?}", v),
    }
}

// === Complete Program Tests ===

#[test]
fn test_factorial() {
    let source = r#"
        fn factorial(n) {
            if n <= 1 {
                return 1
            }
            return n * factorial(n - 1)
        }
        factorial(10)
    "#;
    
    assert_eq!(eval_int(source), 3628800);
}

#[test]
fn test_fibonacci() {
    let source = r#"
        fn fib(n) {
            if n <= 1 {
                return n
            }
            return fib(n - 1) + fib(n - 2)
        }
        fib(15)
    "#;
    
    assert_eq!(eval_int(source), 610);
}

#[test]
fn test_sum_loop() {
    let source = r#"
        let mut sum = 0
        let mut i = 1
        while i <= 100 {
            sum = sum + i
            i = i + 1
        }
        sum
    "#;
    
    assert_eq!(eval_int(source), 5050);
}

#[test]
fn test_array_operations() {
    let source = r#"
        let arr = [5, 2, 8, 1, 9]
        sort(arr)
        arr[0] + arr[4]
    "#;
    
    // After sorting: [1, 2, 5, 8, 9], sum of first and last = 10
    assert_eq!(eval_int(source), 10);
}

#[test]
fn test_table_operations() {
    let source = r#"
        let person = {
            name: "Bob",
            age: 25
        }
        person.age + 10
    "#;
    
    assert_eq!(eval_int(source), 35);
}

#[test]
fn test_nested_functions() {
    let source = r#"
        fn outer(x) {
            fn inner(y) {
                return y * 2
            }
            return inner(x) + 1
        }
        outer(5)
    "#;
    
    assert_eq!(eval_int(source), 11);
}

#[test]
fn test_string_operations() {
    let source = r#"
        let s = "hello"
        let u = upper(s)
        len(u)
    "#;
    
    assert_eq!(eval_int(source), 5);
}

#[test]
fn test_math_functions() {
    let source = r#"
        let a = abs(-10)
        let b = floor(3.7)
        let c = ceil(2.1)
        a + b + c
    "#;
    
    // 10 + 3 + 3 = 16
    assert_eq!(eval_int(source), 16);
}

#[test]
fn test_conditional_expressions() {
    let source = r#"
        let x = 5
        let result = if x > 3 { 100 } else { 200 }
        result
    "#;
    
    assert_eq!(eval_int(source), 100);
}

#[test]
fn test_logical_operators() {
    let source = r#"
        let a = true and false
        let b = true or false
        let c = not a
        if b and c { 1 } else { 0 }
    "#;
    
    assert_eq!(eval_int(source), 1);
}

#[test]
fn test_comparison_operators() {
    let source = r#"
        let count = 0
        let mut c = 0
        if 1 < 2 { c = c + 1 }
        if 2 <= 2 { c = c + 1 }
        if 3 > 2 { c = c + 1 }
        if 2 >= 2 { c = c + 1 }
        if 1 != 2 { c = c + 1 }
        if 2 == 2 { c = c + 1 }
        c
    "#;
    
    assert_eq!(eval_int(source), 6);
}

#[test]
fn test_bitwise_operators() {
    let source = r#"
        let a = 6 & 3
        let b = 4 | 2
        let c = 5 ^ 3
        let d = 1 << 4
        let e = 16 >> 2
        a + b + c + d + e
    "#;
    
    // 2 + 6 + 6 + 16 + 4 = 34
    assert_eq!(eval_int(source), 34);
}

#[test]
fn test_string_concatenation() {
    let source = r#"
        "hello" ++ " " ++ "world"
    "#;
    
    assert_eq!(eval_string(source), "hello world");
}

#[test]
fn test_array_push_pop() {
    let source = r#"
        let arr = [1, 2]
        push(arr, 3)
        push(arr, 4)
        pop(arr)
        len(arr)
    "#;
    
    assert_eq!(eval_int(source), 3);
}

#[test]
fn test_table_keys_values() {
    let source = r#"
        let t = { a: 1, b: 2, c: 3 }
        len(keys(t)) + len(values(t))
    "#;
    
    assert_eq!(eval_int(source), 6);
}

#[test]
fn test_type_function() {
    let source = r#"
        let a = type(42)
        let b = type(3.14)
        let c = type("hi")
        a ++ ", " ++ b ++ ", " ++ c
    "#;
    
    assert_eq!(eval_string(source), "int, float, string");
}

#[test]
fn test_min_max() {
    let source = r#"
        min(5, 2, 8, 1) + max(5, 2, 8, 1)
    "#;
    
    assert_eq!(eval_int(source), 9);
}

#[test]
fn test_contains_function() {
    let source = r#"
        let a = contains("hello", "ell")
        let b = contains("hello", "xyz")
        if a and not b { 1 } else { 0 }
    "#;
    
    assert_eq!(eval_int(source), 1);
}

#[test]
fn test_replace_function() {
    let source = r#"
        replace("hello world", "world", "rust")
    "#;
    
    assert_eq!(eval_string(source), "hello rust");
}

#[test]
fn test_split_function() {
    let source = r#"
        let parts = split("a,b,c", ",")
        len(parts)
    "#;
    
    assert_eq!(eval_int(source), 3);
}

#[test]
fn test_power_operator() {
    let source = r#"
        2 ** 10
    "#;
    
    assert_eq!(eval_int(source), 1024);
}

#[test]
fn test_modulo_operator() {
    let source = r#"
        17 % 5
    "#;
    
    assert_eq!(eval_int(source), 2);
}

#[test]
fn test_negative_numbers() {
    let source = r#"
        let a = -5
        let b = --5
        a + b
    "#;
    
    assert_eq!(eval_int(source), 0);
}

#[test]
fn test_float_arithmetic() {
    let source = r#"
        let a = 1.5 + 2.5
        floor(a)
    "#;
    
    assert_eq!(eval_int(source), 4);
}

#[test]
fn test_gc_pressure() {
    let source = r#"
        let mut i = 0
        while i < 100 {
            let temp = [i, i+1, i+2]
            i = i + 1
        }
        i
    "#;
    
    assert_eq!(eval_int(source), 100);
}

// === Performance Tests ===

#[test]
fn test_performance_loop() {
    use std::time::Instant;
    
    let source = r#"
        let mut sum = 0
        let mut i = 0
        while i < 10000 {
            sum = sum + i
            i = i + 1
        }
        sum
    "#;
    
    let start = Instant::now();
    let result = eval_int(source);
    let elapsed = start.elapsed();
    
    assert_eq!(result, 49995000);
    assert!(elapsed.as_secs() < 5, "Loop took too long: {:?}", elapsed);
}
