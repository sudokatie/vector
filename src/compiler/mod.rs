//! Bytecode compiler

pub mod opcode;
pub mod chunk;

pub use chunk::{Chunk, Function, Module};
pub use opcode::OpCode;

use crate::parser::Stmt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CompileError {
    #[error("Undefined variable '{0}'")]
    UndefinedVariable(String),

    #[error("Too many constants in chunk")]
    TooManyConstants,

    #[error("Too many local variables")]
    TooManyLocals,
}

pub struct Compiler {
    chunk: Chunk,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            chunk: Chunk::new(),
        }
    }

    pub fn compile(&mut self, _stmts: &[Stmt]) -> Result<Module, CompileError> {
        // TODO: Implement compilation
        Ok(Module {
            main: Function {
                name: "main".to_string(),
                arity: 0,
                chunk: std::mem::take(&mut self.chunk),
            },
            functions: Vec::new(),
        })
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}
