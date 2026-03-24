//! Call frame and upvalue management

use super::Value;
use crate::compiler::Function;
use std::rc::Rc;

/// A single call frame on the call stack
#[derive(Debug)]
pub struct CallFrame {
    pub function: Rc<Function>,
    pub ip: usize,
    pub base: usize,
    pub registers: [Value; 256],
}

impl CallFrame {
    pub fn new(function: Rc<Function>, base: usize) -> Self {
        Self {
            function,
            ip: 0,
            base,
            registers: std::array::from_fn(|_| Value::Nil),
        }
    }

    pub fn get_register(&self, index: u8) -> &Value {
        &self.registers[index as usize]
    }

    pub fn set_register(&mut self, index: u8, value: Value) {
        self.registers[index as usize] = value;
    }
}

/// An upvalue (captured variable)
#[derive(Debug)]
pub enum Upvalue {
    Open(usize),      // Index into stack
    Closed(Value),    // Closed over value
}

impl Upvalue {
    pub fn is_open(&self) -> bool {
        matches!(self, Upvalue::Open(_))
    }

    pub fn close(&mut self, value: Value) {
        *self = Upvalue::Closed(value);
    }
}
