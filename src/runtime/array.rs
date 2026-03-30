//! Array type runtime support

use crate::vm::{Value, RuntimeError};
use std::rc::Rc;
use std::cell::RefCell;

/// Push value(s) to array, returns new length
pub fn push(arr: &Rc<RefCell<Vec<Value>>>, values: &[Value]) -> Value {
    let mut arr = arr.borrow_mut();
    for v in values {
        arr.push(v.clone());
    }
    Value::Int(arr.len() as i64)
}

/// Pop value from array end
pub fn pop(arr: &Rc<RefCell<Vec<Value>>>) -> Value {
    arr.borrow_mut().pop().unwrap_or(Value::Nil)
}

/// Insert value at index
pub fn insert(arr: &Rc<RefCell<Vec<Value>>>, index: i64, value: Value) -> Result<Value, RuntimeError> {
    let mut arr = arr.borrow_mut();
    let len = arr.len() as i64;
    
    // Allow negative indices
    let idx = if index < 0 {
        (len + index).max(0) as usize
    } else {
        index.min(len) as usize
    };
    
    arr.insert(idx, value);
    Ok(Value::Nil)
}

/// Remove value at index
pub fn remove(arr: &Rc<RefCell<Vec<Value>>>, index: i64) -> Result<Value, RuntimeError> {
    let mut arr = arr.borrow_mut();
    let len = arr.len() as i64;
    
    if len == 0 {
        return Err(RuntimeError::IndexOutOfBounds(index));
    }
    
    // Allow negative indices
    let idx = if index < 0 {
        let i = len + index;
        if i < 0 {
            return Err(RuntimeError::IndexOutOfBounds(index));
        }
        i as usize
    } else {
        if index >= len {
            return Err(RuntimeError::IndexOutOfBounds(index));
        }
        index as usize
    };
    
    Ok(arr.remove(idx))
}

/// Sort array in place
pub fn sort(arr: &Rc<RefCell<Vec<Value>>>) -> Value {
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
    Value::Nil
}

/// Reverse array in place
pub fn reverse(arr: &Rc<RefCell<Vec<Value>>>) -> Value {
    arr.borrow_mut().reverse();
    Value::Nil
}

/// Map function over array, returning new array
pub fn map(arr: &Rc<RefCell<Vec<Value>>>, func: impl Fn(&Value) -> Result<Value, RuntimeError>) -> Result<Value, RuntimeError> {
    let arr = arr.borrow();
    let mut result = Vec::with_capacity(arr.len());
    for v in arr.iter() {
        result.push(func(v)?);
    }
    Ok(Value::Array(Rc::new(RefCell::new(result))))
}

/// Filter array by predicate, returning new array
pub fn filter(arr: &Rc<RefCell<Vec<Value>>>, pred: impl Fn(&Value) -> Result<bool, RuntimeError>) -> Result<Value, RuntimeError> {
    let arr = arr.borrow();
    let mut result = Vec::new();
    for v in arr.iter() {
        if pred(v)? {
            result.push(v.clone());
        }
    }
    Ok(Value::Array(Rc::new(RefCell::new(result))))
}

/// Reduce array to single value
pub fn reduce(arr: &Rc<RefCell<Vec<Value>>>, init: Value, func: impl Fn(&Value, &Value) -> Result<Value, RuntimeError>) -> Result<Value, RuntimeError> {
    let arr = arr.borrow();
    let mut acc = init;
    for v in arr.iter() {
        acc = func(&acc, v)?;
    }
    Ok(acc)
}

/// Get length of array
pub fn len(arr: &Rc<RefCell<Vec<Value>>>) -> Value {
    Value::Int(arr.borrow().len() as i64)
}

/// Check if array is empty
pub fn is_empty(arr: &Rc<RefCell<Vec<Value>>>) -> Value {
    Value::Bool(arr.borrow().is_empty())
}

/// Get first element
pub fn first(arr: &Rc<RefCell<Vec<Value>>>) -> Value {
    arr.borrow().first().cloned().unwrap_or(Value::Nil)
}

/// Get last element
pub fn last(arr: &Rc<RefCell<Vec<Value>>>) -> Value {
    arr.borrow().last().cloned().unwrap_or(Value::Nil)
}

/// Find index of value (-1 if not found)
pub fn index_of(arr: &Rc<RefCell<Vec<Value>>>, value: &Value) -> Value {
    match arr.borrow().iter().position(|v| v == value) {
        Some(idx) => Value::Int(idx as i64),
        None => Value::Int(-1),
    }
}

/// Check if array contains value
pub fn contains(arr: &Rc<RefCell<Vec<Value>>>, value: &Value) -> Value {
    Value::Bool(arr.borrow().contains(value))
}

/// Join array elements into string
pub fn join(arr: &Rc<RefCell<Vec<Value>>>, sep: &str) -> Value {
    let arr = arr.borrow();
    let parts: Vec<String> = arr.iter().map(|v| format!("{}", v)).collect();
    Value::String(parts.join(sep))
}

/// Get slice of array
pub fn slice(arr: &Rc<RefCell<Vec<Value>>>, start: i64, end: i64) -> Value {
    let arr = arr.borrow();
    let len = arr.len() as i64;
    
    let start = if start < 0 { (len + start).max(0) as usize } else { start as usize };
    let end = if end < 0 { (len + end).max(0) as usize } else { end.min(len) as usize };
    
    if start >= end || start >= arr.len() {
        return Value::Array(Rc::new(RefCell::new(Vec::new())));
    }
    
    let slice: Vec<Value> = arr[start..end].to_vec();
    Value::Array(Rc::new(RefCell::new(slice)))
}

/// Clone/copy array (shallow)
pub fn clone_arr(arr: &Rc<RefCell<Vec<Value>>>) -> Value {
    let arr = arr.borrow();
    Value::Array(Rc::new(RefCell::new(arr.clone())))
}

/// Concatenate two arrays
pub fn concat(arr1: &Rc<RefCell<Vec<Value>>>, arr2: &Rc<RefCell<Vec<Value>>>) -> Value {
    let a1 = arr1.borrow();
    let a2 = arr2.borrow();
    let mut result = a1.clone();
    result.extend(a2.iter().cloned());
    Value::Array(Rc::new(RefCell::new(result)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_arr(vals: Vec<Value>) -> Rc<RefCell<Vec<Value>>> {
        Rc::new(RefCell::new(vals))
    }

    #[test]
    fn test_push_pop() {
        let arr = make_arr(vec![Value::Int(1)]);
        push(&arr, &[Value::Int(2)]);
        assert_eq!(arr.borrow().len(), 2);
        assert_eq!(pop(&arr), Value::Int(2));
        assert_eq!(arr.borrow().len(), 1);
    }

    #[test]
    fn test_insert_remove() {
        let arr = make_arr(vec![Value::Int(1), Value::Int(3)]);
        insert(&arr, 1, Value::Int(2)).unwrap();
        assert_eq!(arr.borrow().len(), 3);
        assert_eq!(remove(&arr, 1).unwrap(), Value::Int(2));
        assert_eq!(arr.borrow().len(), 2);
    }

    #[test]
    fn test_sort() {
        let arr = make_arr(vec![Value::Int(3), Value::Int(1), Value::Int(2)]);
        sort(&arr);
        let arr = arr.borrow();
        assert_eq!(arr[0], Value::Int(1));
        assert_eq!(arr[1], Value::Int(2));
        assert_eq!(arr[2], Value::Int(3));
    }

    #[test]
    fn test_reverse() {
        let arr = make_arr(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        reverse(&arr);
        let arr = arr.borrow();
        assert_eq!(arr[0], Value::Int(3));
        assert_eq!(arr[2], Value::Int(1));
    }

    #[test]
    fn test_contains() {
        let arr = make_arr(vec![Value::Int(1), Value::Int(2)]);
        assert_eq!(contains(&arr, &Value::Int(1)), Value::Bool(true));
        assert_eq!(contains(&arr, &Value::Int(5)), Value::Bool(false));
    }
}
