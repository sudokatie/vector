//! Virtual machine for executing bytecode

pub mod value;
pub mod frame;
pub mod execute;

pub use value::Value;
pub use frame::CallFrame;

use crate::compiler::Function;
use thiserror::Error;
use std::rc::Rc;

#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error("Type error: expected {expected}, got {got}")]
    TypeError { expected: String, got: String },

    #[error("Stack overflow")]
    StackOverflow,

    #[error("Stack underflow")]
    StackUnderflow,

    #[error("Undefined variable '{0}'")]
    UndefinedVariable(String),

    #[error("Division by zero")]
    DivisionByZero,

    #[error("Index out of bounds: {0}")]
    IndexOutOfBounds(i64),

    #[error("Invalid opcode: {0}")]
    InvalidOpcode(u8),

    #[error("Invalid constant index: {0}")]
    InvalidConstant(u16),

    #[error("Unexpected end of bytecode")]
    UnexpectedEnd,

    #[error("Not callable: {0}")]
    NotCallable(String),

    #[error("Arity mismatch: expected {expected}, got {got}")]
    ArityMismatch { expected: u8, got: u8 },

    #[error("Not implemented: {0}")]
    NotImplemented(String),
}

/// The Vector virtual machine
pub struct VM {
    frames: Vec<CallFrame>,
    functions: Vec<Rc<Function>>,
    globals: fnv::FnvHashMap<String, Value>,
    return_register: u8,
}

impl VM {
    pub fn new() -> Self {
        let mut vm = Self {
            frames: Vec::with_capacity(64),
            functions: Vec::new(),
            globals: fnv::FnvHashMap::default(),
            return_register: 0,
        };
        vm.register_stdlib();
        vm
    }

    fn register_stdlib(&mut self) {
        // === Core ===

        // Print function
        self.globals.insert("print".to_string(), Value::NativeFunction(|args| {
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    print!(" ");
                }
                print!("{}", arg);
            }
            println!();
            Ok(Value::Nil)
        }));

        // Type function
        self.globals.insert("type".to_string(), Value::NativeFunction(|args| {
            if args.is_empty() {
                return Ok(Value::String("nil".to_string()));
            }
            Ok(Value::String(args[0].type_name().to_string()))
        }));

        // Len function
        self.globals.insert("len".to_string(), Value::NativeFunction(|args| {
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
        }));

        // Assert function
        self.globals.insert("assert".to_string(), Value::NativeFunction(|args| {
            if args.is_empty() || !args[0].is_truthy() {
                let msg = args.get(1)
                    .map(|v| format!("{}", v))
                    .unwrap_or_else(|| "assertion failed".to_string());
                panic!("{}", msg);
            }
            Ok(Value::Nil)
        }));

        // Error function
        self.globals.insert("error".to_string(), Value::NativeFunction(|args| {
            let msg = args.first()
                .map(|v| format!("{}", v))
                .unwrap_or_else(|| "error".to_string());
            panic!("{}", msg);
        }));

        // === String functions ===

        self.globals.insert("str".to_string(), Value::NativeFunction(|args| {
            if args.is_empty() {
                return Ok(Value::String(String::new()));
            }
            Ok(Value::String(format!("{}", args[0])))
        }));

        self.globals.insert("upper".to_string(), Value::NativeFunction(|args| {
            match args.first() {
                Some(Value::String(s)) => Ok(Value::String(s.to_uppercase())),
                Some(v) => Err(RuntimeError::TypeError {
                    expected: "string".to_string(),
                    got: v.type_name().to_string(),
                }),
                None => Ok(Value::String(String::new())),
            }
        }));

        self.globals.insert("lower".to_string(), Value::NativeFunction(|args| {
            match args.first() {
                Some(Value::String(s)) => Ok(Value::String(s.to_lowercase())),
                Some(v) => Err(RuntimeError::TypeError {
                    expected: "string".to_string(),
                    got: v.type_name().to_string(),
                }),
                None => Ok(Value::String(String::new())),
            }
        }));

        self.globals.insert("trim".to_string(), Value::NativeFunction(|args| {
            match args.first() {
                Some(Value::String(s)) => Ok(Value::String(s.trim().to_string())),
                Some(v) => Err(RuntimeError::TypeError {
                    expected: "string".to_string(),
                    got: v.type_name().to_string(),
                }),
                None => Ok(Value::String(String::new())),
            }
        }));

        // === Math functions ===

        self.globals.insert("abs".to_string(), Value::NativeFunction(|args| {
            match args.first() {
                Some(Value::Int(n)) => Ok(Value::Int(n.abs())),
                Some(Value::Float(n)) => Ok(Value::Float(n.abs())),
                Some(v) => Err(RuntimeError::TypeError {
                    expected: "number".to_string(),
                    got: v.type_name().to_string(),
                }),
                None => Ok(Value::Int(0)),
            }
        }));

        self.globals.insert("floor".to_string(), Value::NativeFunction(|args| {
            match args.first() {
                Some(Value::Int(n)) => Ok(Value::Int(*n)),
                Some(Value::Float(n)) => Ok(Value::Int(n.floor() as i64)),
                Some(v) => Err(RuntimeError::TypeError {
                    expected: "number".to_string(),
                    got: v.type_name().to_string(),
                }),
                None => Ok(Value::Int(0)),
            }
        }));

        self.globals.insert("ceil".to_string(), Value::NativeFunction(|args| {
            match args.first() {
                Some(Value::Int(n)) => Ok(Value::Int(*n)),
                Some(Value::Float(n)) => Ok(Value::Int(n.ceil() as i64)),
                Some(v) => Err(RuntimeError::TypeError {
                    expected: "number".to_string(),
                    got: v.type_name().to_string(),
                }),
                None => Ok(Value::Int(0)),
            }
        }));

        self.globals.insert("sqrt".to_string(), Value::NativeFunction(|args| {
            match args.first() {
                Some(Value::Int(n)) => Ok(Value::Float((*n as f64).sqrt())),
                Some(Value::Float(n)) => Ok(Value::Float(n.sqrt())),
                Some(v) => Err(RuntimeError::TypeError {
                    expected: "number".to_string(),
                    got: v.type_name().to_string(),
                }),
                None => Ok(Value::Float(0.0)),
            }
        }));

        self.globals.insert("min".to_string(), Value::NativeFunction(|args| {
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
        }));

        self.globals.insert("max".to_string(), Value::NativeFunction(|args| {
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
        }));

        // === Array functions ===

        self.globals.insert("push".to_string(), Value::NativeFunction(|args| {
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
        }));

        self.globals.insert("pop".to_string(), Value::NativeFunction(|args| {
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
        }));
    }
}

impl Default for VM {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_creation() {
        let vm = VM::new();
        assert!(vm.globals.contains_key("print"));
        assert!(vm.globals.contains_key("type"));
        assert!(vm.globals.contains_key("len"));
    }
}
