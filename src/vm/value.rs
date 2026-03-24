//! Value representation for the VM

use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::cell::RefCell;

use super::RuntimeError;

/// Native function type
pub type NativeFn = fn(&[Value]) -> Result<Value, RuntimeError>;

/// Runtime value type
#[derive(Clone)]
pub enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Rc<RefCell<Vec<Value>>>),
    Table(Rc<RefCell<fnv::FnvHashMap<Value, Value>>>),
    Function(u16),  // Index into function table
    NativeFunction(NativeFn),
}

impl Value {
    pub fn nil() -> Self {
        Value::Nil
    }

    pub fn bool(b: bool) -> Self {
        Value::Bool(b)
    }

    pub fn int(i: i64) -> Self {
        Value::Int(i)
    }

    pub fn float(f: f64) -> Self {
        Value::Float(f)
    }

    pub fn string(s: impl Into<String>) -> Self {
        Value::String(s.into())
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Nil => false,
            Value::Bool(b) => *b,
            _ => true,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Nil => "nil",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Table(_) => "table",
            Value::Function(_) => "function",
            Value::NativeFunction(_) => "native_function",
        }
    }
}

impl Default for Value {
    fn default() -> Self {
        Value::Nil
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Nil, Value::Nil) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Int(a), Value::Float(b)) => (*a as f64) == *b,
            (Value::Float(a), Value::Int(b)) => *a == (*b as f64),
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => Rc::ptr_eq(a, b),
            (Value::Table(a), Value::Table(b)) => Rc::ptr_eq(a, b),
            (Value::Function(a), Value::Function(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for Value {}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Value::Nil => {}
            Value::Bool(b) => b.hash(state),
            Value::Int(i) => i.hash(state),
            Value::Float(f) => f.to_bits().hash(state),
            Value::String(s) => s.hash(state),
            Value::Array(a) => Rc::as_ptr(a).hash(state),
            Value::Table(t) => Rc::as_ptr(t).hash(state),
            Value::Function(f) => f.hash(state),
            Value::NativeFunction(f) => (*f as usize).hash(state),
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Nil => write!(f, "Nil"),
            Value::Bool(b) => write!(f, "Bool({})", b),
            Value::Int(i) => write!(f, "Int({})", i),
            Value::Float(n) => write!(f, "Float({})", n),
            Value::String(s) => write!(f, "String({:?})", s),
            Value::Array(a) => write!(f, "Array({:?})", a.borrow()),
            Value::Table(_) => write!(f, "Table(...)"),
            Value::Function(idx) => write!(f, "Function({})", idx),
            Value::NativeFunction(_) => write!(f, "NativeFunction"),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Nil => write!(f, "nil"),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Int(i) => write!(f, "{}", i),
            Value::Float(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "{}", s),
            Value::Array(a) => {
                let arr = a.borrow();
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Value::Table(_) => write!(f, "{{...}}"),
            Value::Function(idx) => write!(f, "<function {}>", idx),
            Value::NativeFunction(_) => write!(f, "<native function>"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_types() {
        assert_eq!(Value::Nil.type_name(), "nil");
        assert_eq!(Value::Bool(true).type_name(), "bool");
        assert_eq!(Value::Int(42).type_name(), "int");
        assert_eq!(Value::Float(3.14).type_name(), "float");
        assert_eq!(Value::String("hello".to_string()).type_name(), "string");
    }

    #[test]
    fn test_truthiness() {
        assert!(!Value::Nil.is_truthy());
        assert!(!Value::Bool(false).is_truthy());
        assert!(Value::Bool(true).is_truthy());
        assert!(Value::Int(0).is_truthy());
        assert!(Value::Int(42).is_truthy());
    }

    #[test]
    fn test_equality() {
        assert_eq!(Value::Nil, Value::Nil);
        assert_eq!(Value::Int(42), Value::Int(42));
        assert_eq!(Value::Int(42), Value::Float(42.0));
        assert_ne!(Value::Nil, Value::Bool(false));
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", Value::Nil), "nil");
        assert_eq!(format!("{}", Value::Int(42)), "42");
        assert_eq!(format!("{}", Value::String("hi".to_string())), "hi");
    }
}
