//! Table type runtime support

use crate::vm::{Value, RuntimeError};
use std::rc::Rc;
use std::cell::RefCell;
use fnv::FnvHashMap;

/// Get all keys from table as array
pub fn keys(tbl: &Rc<RefCell<FnvHashMap<Value, Value>>>) -> Value {
    let tbl = tbl.borrow();
    let keys: Vec<Value> = tbl.keys().cloned().collect();
    Value::Array(Rc::new(RefCell::new(keys)))
}

/// Get all values from table as array
pub fn values(tbl: &Rc<RefCell<FnvHashMap<Value, Value>>>) -> Value {
    let tbl = tbl.borrow();
    let values: Vec<Value> = tbl.values().cloned().collect();
    Value::Array(Rc::new(RefCell::new(values)))
}

/// Check if table contains key
pub fn contains(tbl: &Rc<RefCell<FnvHashMap<Value, Value>>>, key: &Value) -> Value {
    Value::Bool(tbl.borrow().contains_key(key))
}

/// Remove key from table, returning the value (or nil)
pub fn remove(tbl: &Rc<RefCell<FnvHashMap<Value, Value>>>, key: &Value) -> Value {
    tbl.borrow_mut().remove(key).unwrap_or(Value::Nil)
}

/// Get value for key, or default if not present
pub fn get_or(tbl: &Rc<RefCell<FnvHashMap<Value, Value>>>, key: &Value, default: Value) -> Value {
    tbl.borrow().get(key).cloned().unwrap_or(default)
}

/// Get number of entries in table
pub fn len(tbl: &Rc<RefCell<FnvHashMap<Value, Value>>>) -> Value {
    Value::Int(tbl.borrow().len() as i64)
}

/// Check if table is empty
pub fn is_empty(tbl: &Rc<RefCell<FnvHashMap<Value, Value>>>) -> Value {
    Value::Bool(tbl.borrow().is_empty())
}

/// Clear all entries from table
pub fn clear(tbl: &Rc<RefCell<FnvHashMap<Value, Value>>>) -> Value {
    tbl.borrow_mut().clear();
    Value::Nil
}

/// Get entries as array of [key, value] pairs
pub fn entries(tbl: &Rc<RefCell<FnvHashMap<Value, Value>>>) -> Value {
    let tbl = tbl.borrow();
    let entries: Vec<Value> = tbl.iter().map(|(k, v)| {
        let pair = vec![k.clone(), v.clone()];
        Value::Array(Rc::new(RefCell::new(pair)))
    }).collect();
    Value::Array(Rc::new(RefCell::new(entries)))
}

/// Merge another table into this one (modifies in place)
pub fn merge(tbl: &Rc<RefCell<FnvHashMap<Value, Value>>>, other: &Rc<RefCell<FnvHashMap<Value, Value>>>) -> Value {
    let other = other.borrow();
    let mut tbl = tbl.borrow_mut();
    for (k, v) in other.iter() {
        tbl.insert(k.clone(), v.clone());
    }
    Value::Nil
}

/// Clone/copy table (shallow)
pub fn clone_tbl(tbl: &Rc<RefCell<FnvHashMap<Value, Value>>>) -> Value {
    let tbl = tbl.borrow();
    Value::Table(Rc::new(RefCell::new(tbl.clone())))
}

/// Set value for key (returns previous value or nil)
pub fn set(tbl: &Rc<RefCell<FnvHashMap<Value, Value>>>, key: Value, value: Value) -> Value {
    tbl.borrow_mut().insert(key, value).unwrap_or(Value::Nil)
}

/// Get value for key (returns nil if not present)
pub fn get(tbl: &Rc<RefCell<FnvHashMap<Value, Value>>>, key: &Value) -> Value {
    tbl.borrow().get(key).cloned().unwrap_or(Value::Nil)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tbl() -> Rc<RefCell<FnvHashMap<Value, Value>>> {
        let mut map = FnvHashMap::default();
        map.insert(Value::String("a".to_string()), Value::Int(1));
        map.insert(Value::String("b".to_string()), Value::Int(2));
        Rc::new(RefCell::new(map))
    }

    #[test]
    fn test_keys() {
        let tbl = make_tbl();
        match keys(&tbl) {
            Value::Array(arr) => {
                assert_eq!(arr.borrow().len(), 2);
            }
            _ => panic!("Expected array"),
        }
    }

    #[test]
    fn test_values() {
        let tbl = make_tbl();
        match values(&tbl) {
            Value::Array(arr) => {
                assert_eq!(arr.borrow().len(), 2);
            }
            _ => panic!("Expected array"),
        }
    }

    #[test]
    fn test_contains() {
        let tbl = make_tbl();
        assert_eq!(contains(&tbl, &Value::String("a".to_string())), Value::Bool(true));
        assert_eq!(contains(&tbl, &Value::String("z".to_string())), Value::Bool(false));
    }

    #[test]
    fn test_remove() {
        let tbl = make_tbl();
        assert_eq!(remove(&tbl, &Value::String("a".to_string())), Value::Int(1));
        assert_eq!(tbl.borrow().len(), 1);
    }

    #[test]
    fn test_len() {
        let tbl = make_tbl();
        assert_eq!(len(&tbl), Value::Int(2));
    }

    #[test]
    fn test_clear() {
        let tbl = make_tbl();
        clear(&tbl);
        assert_eq!(tbl.borrow().len(), 0);
    }
}
