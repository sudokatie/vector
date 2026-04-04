//! Standard library functions

use crate::vm::{Value, RuntimeError};
use std::rc::Rc;
use std::cell::RefCell;
use std::io::{self, BufRead, Write};
use std::fs;

// ============================================================================
// Core functions
// ============================================================================

pub fn print_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            print!(" ");
        }
        print!("{}", arg);
    }
    println!();
    Ok(Value::Nil)
}

pub fn type_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Ok(Value::String("nil".to_string()));
    }
    Ok(Value::String(args[0].type_name().to_string()))
}

pub fn len_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Ok(Value::Int(0));
    }
    match &args[0] {
        Value::String(s) => Ok(Value::Int(s.len() as i64)),
        Value::Array(a) => Ok(Value::Int(a.borrow().len() as i64)),
        Value::Table(t) => Ok(Value::Int(t.borrow().len() as i64)),
        v => Err(RuntimeError::TypeError {
            expected: "string, array, or table".to_string(),
            got: v.type_name().to_string(),
        }),
    }
}

pub fn assert_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() || !args[0].is_truthy() {
        let msg = args.get(1)
            .map(|v| format!("{}", v))
            .unwrap_or_else(|| "assertion failed".to_string());
        panic!("{}", msg);
    }
    Ok(Value::Nil)
}

pub fn error_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    let msg = args.first()
        .map(|v| format!("{}", v))
        .unwrap_or_else(|| "error".to_string());
    Err(RuntimeError::UserError(msg))
}

// ============================================================================
// Result methods (for try expression results)
// ============================================================================

/// Check if a Result table represents an error
/// Called as result.is_err() - self is passed as first arg
pub fn result_is_err_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    // First arg is self (the result table)
    match args.first() {
        Some(Value::Table(tbl)) => {
            let borrowed = tbl.borrow();
            // Check if ok field is false
            if let Some(ok_val) = borrowed.get(&Value::String("ok".to_string())) {
                Ok(Value::Bool(!ok_val.is_truthy()))
            } else {
                // No ok field, assume it's not an error
                Ok(Value::Bool(false))
            }
        }
        _ => Ok(Value::Bool(false)),
    }
}

/// Get the error message from a Result table
/// Called as result.err() - self is passed as first arg
pub fn result_err_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    // First arg is self (the result table)
    match args.first() {
        Some(Value::Table(tbl)) => {
            let borrowed = tbl.borrow();
            // Return the _error field if it exists (internal storage for error message)
            if let Some(err_val) = borrowed.get(&Value::String("_error".to_string())) {
                Ok(err_val.clone())
            } else {
                Ok(Value::Nil)
            }
        }
        _ => Ok(Value::Nil),
    }
}

/// Get the value from a Result table (for successful results)
/// Called as result.unwrap() - self is passed as first arg
pub fn result_unwrap_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Table(tbl)) => {
            let borrowed = tbl.borrow();
            if let Some(ok_val) = borrowed.get(&Value::String("ok".to_string())) {
                if ok_val.is_truthy() {
                    if let Some(val) = borrowed.get(&Value::String("value".to_string())) {
                        return Ok(val.clone());
                    }
                }
            }
            Err(RuntimeError::UserError("called unwrap on error result".to_string()))
        }
        _ => Err(RuntimeError::UserError("unwrap called on non-result".to_string())),
    }
}

pub fn str_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Ok(Value::String(String::new()));
    }
    Ok(Value::String(format!("{}", args[0])))
}

// ============================================================================
// String functions
// ============================================================================

pub fn upper_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::String(s)) => Ok(Value::String(s.to_uppercase())),
        Some(v) => Err(RuntimeError::TypeError {
            expected: "string".to_string(),
            got: v.type_name().to_string(),
        }),
        None => Ok(Value::String(String::new())),
    }
}

pub fn lower_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::String(s)) => Ok(Value::String(s.to_lowercase())),
        Some(v) => Err(RuntimeError::TypeError {
            expected: "string".to_string(),
            got: v.type_name().to_string(),
        }),
        None => Ok(Value::String(String::new())),
    }
}

pub fn trim_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::String(s)) => Ok(Value::String(s.trim().to_string())),
        Some(v) => Err(RuntimeError::TypeError {
            expected: "string".to_string(),
            got: v.type_name().to_string(),
        }),
        None => Ok(Value::String(String::new())),
    }
}

pub fn split_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = match args.first() {
        Some(Value::String(s)) => s,
        Some(v) => return Err(RuntimeError::TypeError {
            expected: "string".to_string(),
            got: v.type_name().to_string(),
        }),
        None => return Ok(Value::Array(Rc::new(RefCell::new(Vec::new())))),
    };
    
    let sep = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => " ",
    };
    
    let parts: Vec<Value> = s.split(sep).map(|p| Value::String(p.to_string())).collect();
    Ok(Value::Array(Rc::new(RefCell::new(parts))))
}

pub fn contains_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    match (&args.first(), &args.get(1)) {
        (Some(Value::String(s)), Some(Value::String(sub))) => {
            Ok(Value::Bool(s.contains(sub.as_str())))
        }
        (Some(Value::Array(arr)), Some(val)) => {
            Ok(Value::Bool(arr.borrow().contains(val)))
        }
        (Some(Value::Table(tbl)), Some(key)) => {
            Ok(Value::Bool(tbl.borrow().contains_key(key)))
        }
        (Some(v), _) => Err(RuntimeError::TypeError {
            expected: "string, array, or table".to_string(),
            got: v.type_name().to_string(),
        }),
        _ => Ok(Value::Bool(false)),
    }
}

pub fn replace_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = match args.first() {
        Some(Value::String(s)) => s,
        Some(v) => return Err(RuntimeError::TypeError {
            expected: "string".to_string(),
            got: v.type_name().to_string(),
        }),
        None => return Ok(Value::String(String::new())),
    };
    
    let old = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        _ => return Ok(Value::String(s.clone())),
    };
    
    let new = match args.get(2) {
        Some(Value::String(s)) => s.as_str(),
        _ => "",
    };
    
    Ok(Value::String(s.replace(old, new)))
}

// ============================================================================
// Math functions
// ============================================================================

/// Helper: skip self (table) argument when called as method
/// Returns args unchanged if first arg is not a table
fn skip_self(args: &[Value]) -> &[Value] {
    if let Some(Value::Table(_)) = args.first() {
        &args[1..]
    } else {
        args
    }
}

pub fn abs_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    let args = skip_self(args);
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(n.abs())),
        Some(Value::Float(n)) => Ok(Value::Float(n.abs())),
        Some(v) => Err(RuntimeError::TypeError {
            expected: "number".to_string(),
            got: v.type_name().to_string(),
        }),
        None => Ok(Value::Int(0)),
    }
}

pub fn floor_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    let args = skip_self(args);
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(*n)),
        Some(Value::Float(n)) => Ok(Value::Int(n.floor() as i64)),
        Some(v) => Err(RuntimeError::TypeError {
            expected: "number".to_string(),
            got: v.type_name().to_string(),
        }),
        None => Ok(Value::Int(0)),
    }
}

pub fn ceil_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    let args = skip_self(args);
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Int(*n)),
        Some(Value::Float(n)) => Ok(Value::Int(n.ceil() as i64)),
        Some(v) => Err(RuntimeError::TypeError {
            expected: "number".to_string(),
            got: v.type_name().to_string(),
        }),
        None => Ok(Value::Int(0)),
    }
}

pub fn sqrt_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    let args = skip_self(args);
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).sqrt())),
        Some(Value::Float(n)) => Ok(Value::Float(n.sqrt())),
        Some(v) => Err(RuntimeError::TypeError {
            expected: "number".to_string(),
            got: v.type_name().to_string(),
        }),
        None => Ok(Value::Float(0.0)),
    }
}

pub fn pow_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    let args = skip_self(args);
    let base = match args.first() {
        Some(Value::Int(n)) => *n as f64,
        Some(Value::Float(n)) => *n,
        Some(v) => return Err(RuntimeError::TypeError {
            expected: "number".to_string(),
            got: v.type_name().to_string(),
        }),
        None => return Ok(Value::Float(0.0)),
    };
    
    let exp = match args.get(1) {
        Some(Value::Int(n)) => *n as f64,
        Some(Value::Float(n)) => *n,
        _ => return Ok(Value::Float(base)),
    };
    
    Ok(Value::Float(base.powf(exp)))
}

pub fn sin_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    let args = skip_self(args);
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).sin())),
        Some(Value::Float(n)) => Ok(Value::Float(n.sin())),
        Some(v) => Err(RuntimeError::TypeError {
            expected: "number".to_string(),
            got: v.type_name().to_string(),
        }),
        None => Ok(Value::Float(0.0)),
    }
}

pub fn cos_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    let args = skip_self(args);
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).cos())),
        Some(Value::Float(n)) => Ok(Value::Float(n.cos())),
        Some(v) => Err(RuntimeError::TypeError {
            expected: "number".to_string(),
            got: v.type_name().to_string(),
        }),
        None => Ok(Value::Float(1.0)),
    }
}

pub fn tan_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    let args = skip_self(args);
    match args.first() {
        Some(Value::Int(n)) => Ok(Value::Float((*n as f64).tan())),
        Some(Value::Float(n)) => Ok(Value::Float(n.tan())),
        Some(v) => Err(RuntimeError::TypeError {
            expected: "number".to_string(),
            got: v.type_name().to_string(),
        }),
        None => Ok(Value::Float(0.0)),
    }
}

pub fn min_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    let args = skip_self(args);
    if args.is_empty() {
        return Ok(Value::Nil);
    }
    let mut min = args[0].clone();
    for arg in args.iter().skip(1) {
        match (&min, arg) {
            (Value::Int(a), Value::Int(b)) if b < a => min = arg.clone(),
            (Value::Float(a), Value::Float(b)) if b < a => min = arg.clone(),
            (Value::Int(a), Value::Float(b)) if *b < *a as f64 => min = arg.clone(),
            (Value::Float(a), Value::Int(b)) if (*b as f64) < *a => min = arg.clone(),
            _ => {}
        }
    }
    Ok(min)
}

pub fn max_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    let args = skip_self(args);
    if args.is_empty() {
        return Ok(Value::Nil);
    }
    let mut max = args[0].clone();
    for arg in args.iter().skip(1) {
        match (&max, arg) {
            (Value::Int(a), Value::Int(b)) if b > a => max = arg.clone(),
            (Value::Float(a), Value::Float(b)) if b > a => max = arg.clone(),
            (Value::Int(a), Value::Float(b)) if *b > *a as f64 => max = arg.clone(),
            (Value::Float(a), Value::Int(b)) if (*b as f64) > *a => max = arg.clone(),
            _ => {}
        }
    }
    Ok(max)
}

pub fn random_fn(_args: &[Value]) -> Result<Value, RuntimeError> {
    // Simple LCG random - good enough for basic use
    use std::time::{SystemTime, UNIX_EPOCH};
    static mut SEED: u64 = 0;
    unsafe {
        if SEED == 0 {
            SEED = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64;
        }
        SEED = SEED.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let val = (SEED >> 33) as f64 / (1u64 << 31) as f64;
        Ok(Value::Float(val))
    }
}

pub fn random_int_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    let args = skip_self(args);
    let min = match args.first() {
        Some(Value::Int(n)) => *n,
        _ => 0,
    };
    
    let max = match args.get(1) {
        Some(Value::Int(n)) => *n,
        _ => i64::MAX,
    };
    
    if min >= max {
        return Ok(Value::Int(min));
    }
    
    // Get random float and scale to range
    if let Ok(Value::Float(r)) = random_fn(&[]) {
        let range = (max - min) as f64;
        let val = min + (r * range) as i64;
        Ok(Value::Int(val))
    } else {
        Ok(Value::Int(min))
    }
}

// ============================================================================
// Array functions
// ============================================================================

pub fn push_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Array(arr)) => {
            let mut arr = arr.borrow_mut();
            for arg in args.iter().skip(1) {
                arr.push(arg.clone());
            }
            Ok(Value::Int(arr.len() as i64))
        }
        Some(v) => Err(RuntimeError::TypeError {
            expected: "array".to_string(),
            got: v.type_name().to_string(),
        }),
        None => Err(RuntimeError::TypeError {
            expected: "array".to_string(),
            got: "nil".to_string(),
        }),
    }
}

pub fn pop_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Array(arr)) => {
            let mut arr = arr.borrow_mut();
            Ok(arr.pop().unwrap_or(Value::Nil))
        }
        Some(v) => Err(RuntimeError::TypeError {
            expected: "array".to_string(),
            got: v.type_name().to_string(),
        }),
        None => Ok(Value::Nil),
    }
}

pub fn insert_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    let arr = match args.first() {
        Some(Value::Array(a)) => a,
        Some(v) => return Err(RuntimeError::TypeError {
            expected: "array".to_string(),
            got: v.type_name().to_string(),
        }),
        None => return Ok(Value::Nil),
    };
    
    let idx = match args.get(1) {
        Some(Value::Int(i)) => *i,
        _ => return Ok(Value::Nil),
    };
    
    let val = args.get(2).cloned().unwrap_or(Value::Nil);
    
    let mut arr = arr.borrow_mut();
    let len = arr.len() as i64;
    let idx = if idx < 0 { (len + idx).max(0) as usize } else { idx.min(len) as usize };
    arr.insert(idx, val);
    Ok(Value::Nil)
}

pub fn remove_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    match (&args.first(), &args.get(1)) {
        (Some(Value::Array(arr)), Some(Value::Int(idx))) => {
            let mut arr = arr.borrow_mut();
            let len = arr.len() as i64;
            let idx_val = *idx;
            let idx = if idx_val < 0 { len + idx_val } else { idx_val };
            if idx < 0 || idx >= len {
                return Err(RuntimeError::IndexOutOfBounds(idx_val));
            }
            Ok(arr.remove(idx as usize))
        }
        (Some(Value::Table(tbl)), Some(key)) => {
            Ok(tbl.borrow_mut().remove(key).unwrap_or(Value::Nil))
        }
        (Some(v), _) => Err(RuntimeError::TypeError {
            expected: "array or table".to_string(),
            got: v.type_name().to_string(),
        }),
        _ => Ok(Value::Nil),
    }
}

pub fn sort_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Array(arr)) => {
            let mut arr = arr.borrow_mut();
            arr.sort_by(|a, b| {
                match (a, b) {
                    (Value::Int(x), Value::Int(y)) => x.cmp(y),
                    (Value::Float(x), Value::Float(y)) => x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
                    (Value::Int(x), Value::Float(y)) => (*x as f64).partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
                    (Value::Float(x), Value::Int(y)) => x.partial_cmp(&(*y as f64)).unwrap_or(std::cmp::Ordering::Equal),
                    (Value::String(x), Value::String(y)) => x.cmp(y),
                    _ => std::cmp::Ordering::Equal,
                }
            });
            Ok(Value::Nil)
        }
        Some(v) => Err(RuntimeError::TypeError {
            expected: "array".to_string(),
            got: v.type_name().to_string(),
        }),
        None => Ok(Value::Nil),
    }
}

pub fn reverse_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Array(arr)) => {
            arr.borrow_mut().reverse();
            Ok(Value::Nil)
        }
        Some(Value::String(s)) => {
            Ok(Value::String(s.chars().rev().collect()))
        }
        Some(v) => Err(RuntimeError::TypeError {
            expected: "array or string".to_string(),
            got: v.type_name().to_string(),
        }),
        None => Ok(Value::Nil),
    }
}

/// Clock function for timing
pub fn clock_fn(_args: &[Value]) -> Result<Value, RuntimeError> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap();
    Ok(Value::Float(duration.as_secs_f64()))
}

// ============================================================================
// Table functions
// ============================================================================

pub fn keys_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Table(tbl)) => {
            let tbl = tbl.borrow();
            let keys: Vec<Value> = tbl.keys().cloned().collect();
            Ok(Value::Array(Rc::new(RefCell::new(keys))))
        }
        Some(v) => Err(RuntimeError::TypeError {
            expected: "table".to_string(),
            got: v.type_name().to_string(),
        }),
        None => Ok(Value::Array(Rc::new(RefCell::new(Vec::new())))),
    }
}

pub fn values_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    match args.first() {
        Some(Value::Table(tbl)) => {
            let tbl = tbl.borrow();
            let values: Vec<Value> = tbl.values().cloned().collect();
            Ok(Value::Array(Rc::new(RefCell::new(values))))
        }
        Some(v) => Err(RuntimeError::TypeError {
            expected: "table".to_string(),
            got: v.type_name().to_string(),
        }),
        None => Ok(Value::Array(Rc::new(RefCell::new(Vec::new())))),
    }
}

pub fn table_contains_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    match (args.first(), args.get(1)) {
        (Some(Value::Table(tbl)), Some(key)) => {
            Ok(Value::Bool(tbl.borrow().contains_key(key)))
        }
        (Some(v), _) => Err(RuntimeError::TypeError {
            expected: "table".to_string(),
            got: v.type_name().to_string(),
        }),
        _ => Ok(Value::Bool(false)),
    }
}

pub fn table_remove_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    match (args.first(), args.get(1)) {
        (Some(Value::Table(tbl)), Some(key)) => {
            Ok(tbl.borrow_mut().remove(key).unwrap_or(Value::Nil))
        }
        (Some(v), _) => Err(RuntimeError::TypeError {
            expected: "table".to_string(),
            got: v.type_name().to_string(),
        }),
        _ => Ok(Value::Nil),
    }
}

// ============================================================================
// IO functions
// ============================================================================

pub fn read_file_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    let args = skip_self(args);
    let path = match args.first() {
        Some(Value::String(s)) => s,
        Some(v) => return Err(RuntimeError::TypeError {
            expected: "string".to_string(),
            got: v.type_name().to_string(),
        }),
        None => return Ok(Value::Nil),
    };
    
    match fs::read_to_string(path) {
        Ok(content) => Ok(Value::String(content)),
        Err(e) => {
            // Return nil on error (or could panic)
            eprintln!("read_file error: {}", e);
            Ok(Value::Nil)
        }
    }
}

pub fn write_file_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    let args = skip_self(args);
    let path = match args.first() {
        Some(Value::String(s)) => s,
        Some(v) => return Err(RuntimeError::TypeError {
            expected: "string".to_string(),
            got: v.type_name().to_string(),
        }),
        None => return Ok(Value::Bool(false)),
    };
    
    let content = match args.get(1) {
        Some(Value::String(s)) => s.as_str(),
        Some(v) => &format!("{}", v),
        None => "",
    };
    
    match fs::write(path, content) {
        Ok(_) => Ok(Value::Bool(true)),
        Err(e) => {
            eprintln!("write_file error: {}", e);
            Ok(Value::Bool(false))
        }
    }
}

pub fn read_line_fn(_args: &[Value]) -> Result<Value, RuntimeError> {
    let stdin = io::stdin();
    let mut line = String::new();
    
    match stdin.lock().read_line(&mut line) {
        Ok(_) => {
            // Remove trailing newline
            if line.ends_with('\n') {
                line.pop();
                if line.ends_with('\r') {
                    line.pop();
                }
            }
            Ok(Value::String(line))
        }
        Err(e) => {
            eprintln!("read_line error: {}", e);
            Ok(Value::Nil)
        }
    }
}

pub fn input_fn(args: &[Value]) -> Result<Value, RuntimeError> {
    // Print prompt if provided
    if let Some(Value::String(prompt)) = args.first() {
        print!("{}", prompt);
        io::stdout().flush().ok();
    }
    read_line_fn(&[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_fn() {
        assert_eq!(type_fn(&[Value::Int(42)]).unwrap(), Value::String("int".to_string()));
        assert_eq!(type_fn(&[Value::String("hi".to_string())]).unwrap(), Value::String("string".to_string()));
    }

    #[test]
    fn test_len_fn() {
        assert_eq!(len_fn(&[Value::String("hello".to_string())]).unwrap(), Value::Int(5));
    }

    #[test]
    fn test_math_fns() {
        assert_eq!(abs_fn(&[Value::Int(-5)]).unwrap(), Value::Int(5));
        assert_eq!(floor_fn(&[Value::Float(3.7)]).unwrap(), Value::Int(3));
        assert_eq!(ceil_fn(&[Value::Float(3.2)]).unwrap(), Value::Int(4));
    }

    #[test]
    fn test_trig_fns() {
        let sin_0 = sin_fn(&[Value::Int(0)]).unwrap();
        match sin_0 {
            Value::Float(f) => assert!(f.abs() < 0.0001),
            _ => panic!("Expected float"),
        }
    }

    #[test]
    fn test_random() {
        let r1 = random_fn(&[]).unwrap();
        let r2 = random_fn(&[]).unwrap();
        match (r1, r2) {
            (Value::Float(a), Value::Float(b)) => {
                assert!(a >= 0.0 && a < 1.0);
                assert!(b >= 0.0 && b < 1.0);
                // Unlikely to be equal
                assert_ne!(a, b);
            }
            _ => panic!("Expected floats"),
        }
    }

    #[test]
    fn test_split() {
        let result = split_fn(&[
            Value::String("a,b,c".to_string()),
            Value::String(",".to_string()),
        ]).unwrap();
        match result {
            Value::Array(arr) => assert_eq!(arr.borrow().len(), 3),
            _ => panic!("Expected array"),
        }
    }

    #[test]
    fn test_replace() {
        let result = replace_fn(&[
            Value::String("hello world".to_string()),
            Value::String("world".to_string()),
            Value::String("rust".to_string()),
        ]).unwrap();
        assert_eq!(result, Value::String("hello rust".to_string()));
    }
}
