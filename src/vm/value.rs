//! Value representation for the VM

use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::cell::RefCell;
use std::any::Any;

use super::RuntimeError;

/// Native function type
pub type NativeFn = fn(&[Value]) -> Result<Value, RuntimeError>;

/// Built-in higher-order functions (need VM access for callbacks)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinHOF {
    Map,
    Filter,
    Reduce,
}

/// Userdata - opaque handle to host data
pub struct Userdata {
    pub data: Box<dyn Any>,
    pub type_name: &'static str,
}

impl Clone for Userdata {
    fn clone(&self) -> Self {
        // Userdata is reference-counted, cloning just increases refcount
        // The actual data is not cloned
        panic!("Userdata cannot be directly cloned - use Rc<RefCell<Userdata>>")
    }
}

impl fmt::Debug for Userdata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Userdata({})", self.type_name)
    }
}

/// Range value (for iteration)
#[derive(Debug, Clone, PartialEq)]
pub struct Range {
    pub start: i64,
    pub end: i64,
    pub inclusive: bool,
}

/// Iterator state
#[derive(Debug, Clone)]
pub enum Iterator {
    Range { current: i64, end: i64, inclusive: bool },
    Array { array: Rc<RefCell<Vec<Value>>>, index: usize },
    Table { keys: Vec<Value>, index: usize },
}

/// Closure with captured upvalues
#[derive(Debug, Clone)]
pub struct Closure {
    pub func_idx: u16,
    pub upvalues: Vec<Rc<RefCell<Upvalue>>>,
}

/// An upvalue (captured variable from enclosing scope)
#[derive(Debug, Clone)]
pub enum Upvalue {
    /// Open upvalue - points to a register in an active frame
    Open { frame_idx: usize, register: u8 },
    /// Closed upvalue - value has been moved here when frame exited
    Closed(Value),
}

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
    Function(u16),  // Index into function table (no upvalues)
    Closure(Rc<Closure>),  // Function with captured upvalues
    NativeFunction(NativeFn),
    BuiltinHOF(BuiltinHOF),  // Higher-order functions (map, filter, reduce)
    Userdata(Rc<RefCell<Userdata>>),
    Range(Range),
    Iterator(Rc<RefCell<Iterator>>),
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
            Value::Function(_) | Value::Closure(_) => "function",
            Value::NativeFunction(_) | Value::BuiltinHOF(_) => "function",
            Value::Userdata(u) => u.borrow().type_name,
            Value::Range(_) => "range",
            Value::Iterator(_) => "iterator",
        }
    }

    /// Deep copy a value (for COPY opcode)
    pub fn deep_copy(&self) -> Self {
        match self {
            // Primitives are Copy
            Value::Nil => Value::Nil,
            Value::Bool(b) => Value::Bool(*b),
            Value::Int(i) => Value::Int(*i),
            Value::Float(f) => Value::Float(*f),
            Value::String(s) => Value::String(s.clone()),
            // Compound types get deep copied
            Value::Array(arr) => {
                let copied: Vec<Value> = arr.borrow().iter().map(|v| v.deep_copy()).collect();
                Value::Array(Rc::new(RefCell::new(copied)))
            }
            Value::Table(tbl) => {
                let copied: fnv::FnvHashMap<Value, Value> = tbl
                    .borrow()
                    .iter()
                    .map(|(k, v)| (k.deep_copy(), v.deep_copy()))
                    .collect();
                Value::Table(Rc::new(RefCell::new(copied)))
            }
            // Functions are not deep copied (they're immutable)
            Value::Function(idx) => Value::Function(*idx),
            Value::Closure(c) => Value::Closure(Rc::clone(c)),
            Value::NativeFunction(f) => Value::NativeFunction(*f),
            Value::BuiltinHOF(h) => Value::BuiltinHOF(*h),
            // Userdata cannot be deep copied
            Value::Userdata(u) => Value::Userdata(Rc::clone(u)),
            // Ranges are copied
            Value::Range(r) => Value::Range(r.clone()),
            // Iterators are reference counted
            Value::Iterator(it) => Value::Iterator(Rc::clone(it)),
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
            (Value::Closure(a), Value::Closure(b)) => Rc::ptr_eq(a, b),
            (Value::BuiltinHOF(a), Value::BuiltinHOF(b)) => a == b,
            (Value::Userdata(a), Value::Userdata(b)) => Rc::ptr_eq(a, b),
            (Value::Range(a), Value::Range(b)) => a == b,
            (Value::Iterator(a), Value::Iterator(b)) => Rc::ptr_eq(a, b),
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
            Value::Closure(c) => Rc::as_ptr(c).hash(state),
            Value::NativeFunction(f) => (*f as usize).hash(state),
            Value::BuiltinHOF(h) => h.hash(state),
            Value::Userdata(u) => Rc::as_ptr(u).hash(state),
            Value::Range(r) => {
                r.start.hash(state);
                r.end.hash(state);
                r.inclusive.hash(state);
            }
            Value::Iterator(it) => Rc::as_ptr(it).hash(state),
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
            Value::Closure(c) => write!(f, "Closure(func={}, upvalues={})", c.func_idx, c.upvalues.len()),
            Value::NativeFunction(_) => write!(f, "NativeFunction"),
            Value::BuiltinHOF(h) => write!(f, "BuiltinHOF({:?})", h),
            Value::Userdata(u) => write!(f, "Userdata({})", u.borrow().type_name),
            Value::Range(r) => {
                if r.inclusive {
                    write!(f, "Range({}..={})", r.start, r.end)
                } else {
                    write!(f, "Range({}..{})", r.start, r.end)
                }
            }
            Value::Iterator(_) => write!(f, "Iterator(...)"),
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
            Value::Closure(c) => write!(f, "<closure {}>", c.func_idx),
            Value::NativeFunction(_) => write!(f, "<native function>"),
            Value::BuiltinHOF(h) => write!(f, "<builtin {:?}>", h),
            Value::Userdata(u) => write!(f, "<userdata {}>", u.borrow().type_name),
            Value::Range(r) => {
                if r.inclusive {
                    write!(f, "{}..={}", r.start, r.end)
                } else {
                    write!(f, "{}..{}", r.start, r.end)
                }
            }
            Value::Iterator(_) => write!(f, "<iterator>"),
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
