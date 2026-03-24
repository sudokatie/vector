//! Vector - JIT-compiled scripting language
//!
//! A dynamically-typed scripting language with Lua-like semantics,
//! register-based VM, and JIT compilation via Cranelift.

pub mod lexer;
pub mod parser;
pub mod compiler;
pub mod vm;
pub mod gc;
pub mod jit;
pub mod runtime;

use thiserror::Error;

/// High-level Vector interpreter API
pub struct Vector {
    vm: vm::VM,
}

impl Vector {
    /// Create a new Vector interpreter
    pub fn new() -> Self {
        Self {
            vm: vm::VM::new(),
        }
    }

    /// Create a new Vector interpreter without JIT (interpreter only)
    pub fn new_without_jit() -> Self {
        Self {
            vm: vm::VM::new_without_jit(),
        }
    }

    /// Evaluate source code and return the result
    pub fn eval(&mut self, source: &str) -> Result<vm::Value, VectorError> {
        let tokens = lexer::Lexer::new(source);
        let mut parser = parser::Parser::new(tokens);
        let ast = parser.parse()?;
        let module = compiler::Compiler::new().compile(&ast)?;
        self.vm.run(module).map_err(VectorError::Runtime)
    }

    /// Run a script file
    pub fn run_file(&mut self, path: &str) -> Result<vm::Value, VectorError> {
        let source = std::fs::read_to_string(path)
            .map_err(|e| VectorError::Io(e.to_string()))?;
        self.eval(&source)
    }

    /// Enable or disable JIT compilation
    pub fn set_jit_enabled(&mut self, enabled: bool) {
        self.vm.set_jit_enabled(enabled);
    }

    /// Get JIT statistics
    pub fn jit_stats(&self) -> Option<&jit::JitStats> {
        self.vm.jit_stats()
    }

    /// Get profiler statistics
    pub fn profiler_stats(&self) -> Option<&jit::ProfilerStats> {
        self.vm.profiler_stats()
    }
}

impl Default for Vector {
    fn default() -> Self {
        Self::new()
    }
}

/// Top-level error type for Vector operations
#[derive(Error, Debug)]
pub enum VectorError {
    #[error("Lexer error: {0}")]
    Lexer(#[from] lexer::LexError),

    #[error("Parse error: {0}")]
    Parse(#[from] parser::ParseError),

    #[error("Compile error: {0}")]
    Compile(#[from] compiler::CompileError),

    #[error("Runtime error: {0}")]
    Runtime(#[from] vm::RuntimeError),

    #[error("IO error: {0}")]
    Io(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_creation() {
        let _v = Vector::new();
    }
}
