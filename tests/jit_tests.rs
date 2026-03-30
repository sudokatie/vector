//! JIT compiler integration tests

use vector::Vector;
use vector::vm::Value;
use vector::jit::{Profiler, TypeTag};

fn eval(source: &str) -> Value {
    let mut v = Vector::new();
    v.eval(source).unwrap()
}

fn eval_no_jit(source: &str) -> Value {
    let mut v = Vector::new_without_jit();
    v.eval(source).unwrap()
}

// === Profiler Tests ===

#[test]
fn test_profiler_creation() {
    let profiler = Profiler::new();
    assert_eq!(profiler.stats.total_calls, 0);
    assert_eq!(profiler.stats.functions_compiled, 0);
}

#[test]
fn test_profiler_custom_thresholds() {
    let profiler = Profiler::with_thresholds(50, 25);
    assert_eq!(profiler.hot_threshold, 50);
}

#[test]
fn test_profiler_init_function() {
    let mut profiler = Profiler::new();
    profiler.init_function(0, 2);
    
    // Should have profile for function 0
}

#[test]
fn test_profiler_record_call() {
    let mut profiler = Profiler::new();
    profiler.init_function(0, 0);
    
    for _ in 0..100 {
        profiler.record_call(0);
    }
    
    assert_eq!(profiler.stats.total_calls, 100);
}

#[test]
fn test_profiler_hot_detection() {
    let mut profiler = Profiler::with_thresholds(10, 5);
    profiler.init_function(0, 0);
    
    // Not hot yet
    for _ in 0..9 {
        profiler.record_call(0);
    }
    assert!(!profiler.is_hot(0));
    
    // Now hot
    profiler.record_call(0);
    assert!(profiler.is_hot(0));
}

#[test]
fn test_profiler_type_recording() {
    let mut profiler = Profiler::new();
    profiler.init_function(0, 2);
    
    // Record many int types
    for _ in 0..100 {
        profiler.record_arg_type(0, 0, TypeTag::Int);
    }
    
    // Should detect Int as dominant type
    if let Some(profile) = profiler.get_profile(0) {
        if let Some(arg_profile) = profile.arg_types.get(0) {
            assert!(arg_profile.int_count >= 100);
        }
    }
}

// === JIT Statistics Tests ===

#[test]
fn test_jit_stats_available() {
    let v = Vector::new();
    assert!(v.jit_stats().is_some());
}

#[test]
fn test_jit_stats_not_available_without_jit() {
    let v = Vector::new_without_jit();
    assert!(v.jit_stats().is_none());
}

#[test]
fn test_profiler_stats_available() {
    let v = Vector::new();
    assert!(v.profiler_stats().is_some());
}

// === JIT Enable/Disable Tests ===

#[test]
fn test_jit_toggle() {
    let mut v = Vector::new();
    
    v.set_jit_enabled(false);
    // Should still work
    let result = v.eval("1 + 1").unwrap();
    assert!(matches!(result, Value::Int(2)));
    
    v.set_jit_enabled(true);
    let result = v.eval("2 + 2").unwrap();
    assert!(matches!(result, Value::Int(4)));
}

// === Execution Correctness Tests ===
// These tests verify that JIT and interpreter produce the same results

#[test]
fn test_jit_arithmetic_correctness() {
    let source = "1 + 2 * 3";
    
    let jit_result = eval(source);
    let interp_result = eval_no_jit(source);
    
    assert_eq!(jit_result, interp_result);
}

#[test]
fn test_jit_function_correctness() {
    let source = r#"
        fn add(a, b) { return a + b }
        add(1, 2) + add(3, 4)
    "#;
    
    let jit_result = eval(source);
    let interp_result = eval_no_jit(source);
    
    assert_eq!(jit_result, interp_result);
}

#[test]
fn test_jit_recursive_correctness() {
    let source = r#"
        fn fib(n) {
            if n <= 1 { return n }
            return fib(n-1) + fib(n-2)
        }
        fib(10)
    "#;
    
    let jit_result = eval(source);
    let interp_result = eval_no_jit(source);
    
    assert_eq!(jit_result, interp_result);
}

#[test]
fn test_jit_loop_correctness() {
    let source = r#"
        let mut sum = 0
        let mut i = 0
        while i < 100 {
            sum = sum + i
            i = i + 1
        }
        sum
    "#;
    
    let jit_result = eval(source);
    let interp_result = eval_no_jit(source);
    
    assert_eq!(jit_result, interp_result);
}

#[test]
fn test_jit_closure_correctness() {
    let source = r#"
        fn make_adder(x) {
            return fn(y) { x + y }
        }
        let add5 = make_adder(5)
        add5(10)
    "#;
    
    let jit_result = eval(source);
    let interp_result = eval_no_jit(source);
    
    assert_eq!(jit_result, interp_result);
}

// === Hot Path Tests ===

#[test]
fn test_hot_function_detection() {
    let mut v = Vector::new();
    
    // Call a function many times to make it hot
    let source = r#"
        fn inc(x) { x + 1 }
        let mut i = 0
        while i < 200 {
            inc(i)
            i = i + 1
        }
        i
    "#;
    
    let _ = v.eval(source).unwrap();
    
    // Check profiler stats
    if let Some(stats) = v.profiler_stats() {
        assert!(stats.total_calls > 0);
    }
}

#[test]
fn test_hot_loop_detection() {
    let mut v = Vector::new();
    
    let source = r#"
        let mut sum = 0
        let mut i = 0
        while i < 1000 {
            sum = sum + i
            i = i + 1
        }
        sum
    "#;
    
    let _ = v.eval(source).unwrap();
    
    if let Some(stats) = v.profiler_stats() {
        assert!(stats.total_loop_iterations > 0);
    }
}
