//! String type runtime support

use crate::vm::{Value, RuntimeError};

/// Split a string by separator
pub fn split(s: &str, sep: &str) -> Value {
    let parts: Vec<Value> = s.split(sep).map(|p| Value::String(p.to_string())).collect();
    Value::Array(std::rc::Rc::new(std::cell::RefCell::new(parts)))
}

/// Check if string contains substring
pub fn contains(s: &str, sub: &str) -> Value {
    Value::Bool(s.contains(sub))
}

/// Replace all occurrences of old with new
pub fn replace(s: &str, old: &str, new: &str) -> Value {
    Value::String(s.replace(old, new))
}

/// Convert to uppercase
pub fn upper(s: &str) -> Value {
    Value::String(s.to_uppercase())
}

/// Convert to lowercase
pub fn lower(s: &str) -> Value {
    Value::String(s.to_lowercase())
}

/// Trim whitespace from both ends
pub fn trim(s: &str) -> Value {
    Value::String(s.trim().to_string())
}

/// Get string length
pub fn len(s: &str) -> Value {
    Value::Int(s.len() as i64)
}

/// Check if string starts with prefix
pub fn starts_with(s: &str, prefix: &str) -> Value {
    Value::Bool(s.starts_with(prefix))
}

/// Check if string ends with suffix
pub fn ends_with(s: &str, suffix: &str) -> Value {
    Value::Bool(s.ends_with(suffix))
}

/// Get substring
pub fn substring(s: &str, start: i64, end: i64) -> Result<Value, RuntimeError> {
    let len = s.len() as i64;
    let start = if start < 0 { 0 } else { start as usize };
    let end = if end < 0 || end > len { len as usize } else { end as usize };
    
    if start > end || start > s.len() {
        return Ok(Value::String(String::new()));
    }
    
    Ok(Value::String(s[start..end].to_string()))
}

/// Find index of substring (-1 if not found)
pub fn index_of(s: &str, sub: &str) -> Value {
    match s.find(sub) {
        Some(idx) => Value::Int(idx as i64),
        None => Value::Int(-1),
    }
}

/// Repeat string n times
pub fn repeat(s: &str, n: i64) -> Value {
    if n <= 0 {
        return Value::String(String::new());
    }
    Value::String(s.repeat(n as usize))
}

/// Reverse string
pub fn reverse(s: &str) -> Value {
    Value::String(s.chars().rev().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split() {
        match split("a,b,c", ",") {
            Value::Array(arr) => {
                let arr = arr.borrow();
                assert_eq!(arr.len(), 3);
            }
            _ => panic!("Expected array"),
        }
    }

    #[test]
    fn test_contains() {
        assert_eq!(contains("hello world", "world"), Value::Bool(true));
        assert_eq!(contains("hello world", "xyz"), Value::Bool(false));
    }

    #[test]
    fn test_replace() {
        assert_eq!(replace("hello world", "world", "rust"), Value::String("hello rust".to_string()));
    }

    #[test]
    fn test_upper_lower() {
        assert_eq!(upper("hello"), Value::String("HELLO".to_string()));
        assert_eq!(lower("HELLO"), Value::String("hello".to_string()));
    }

    #[test]
    fn test_trim() {
        assert_eq!(trim("  hello  "), Value::String("hello".to_string()));
    }
}
