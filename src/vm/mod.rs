//! Virtual machine for executing bytecode

pub mod value;
pub mod frame;

pub use value::Value;
pub use frame::CallFrame;

use crate::compiler::Module;
use thiserror::Error;

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
}

/// The Vector virtual machine
pub struct VM {
    frames: Vec<CallFrame>,
    globals: fnv::FnvHashMap<String, Value>,
}

impl VM {
    pub fn new() -> Self {
        Self {
            frames: Vec::with_capacity(64),
            globals: fnv::FnvHashMap::default(),
        }
    }

    pub fn run(&mut self, _module: Module) -> Result<Value, RuntimeError> {
        // TODO: Implement bytecode execution
        Ok(Value::Nil)
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
        let _vm = VM::new();
    }
}
