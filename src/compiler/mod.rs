//! Bytecode compiler

pub mod opcode;
pub mod chunk;
pub mod emit;
pub mod optimize;

pub use chunk::{Chunk, Function, Module, UpvalueInfo};
pub use opcode::OpCode;
pub use optimize::{Optimizer, OptLevel};

use crate::parser::Stmt;
use crate::vm::Value;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CompileError {
    #[error("Undefined variable '{0}'")]
    UndefinedVariable(String),

    #[error("Too many constants in chunk (max 65535)")]
    TooManyConstants,

    #[error("Too many local variables (max 256)")]
    TooManyLocals,

    #[error("Too many upvalues (max 256)")]
    TooManyUpvalues,

    #[error("Invalid assignment target")]
    InvalidAssignmentTarget,

    #[error("Jump too large")]
    JumpTooLarge,

    #[error("Not implemented: {0}")]
    NotImplemented(String),
}

/// Local variable info
#[derive(Debug, Clone)]
struct Local {
    name: String,
    depth: u32,
}

/// Compiler for bytecode generation
pub struct Compiler {
    function: Function,
    functions: Vec<Function>,
    locals: Vec<Local>,
    pub(crate) scope_depth: u32,
    line: u32,
    pub(crate) enclosing: Option<*mut Compiler>,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            function: Function::new("main", 0),
            functions: Vec::new(),
            locals: Vec::new(),
            scope_depth: 0,
            line: 1,
            enclosing: None,
        }
    }

    pub fn new_function(name: String, arity: u8) -> Self {
        Self {
            function: Function::new(name, arity),
            functions: Vec::new(),
            locals: Vec::new(),
            scope_depth: 0,
            line: 1,
            enclosing: None,
        }
    }

    pub fn compile(&mut self, stmts: &[Stmt]) -> Result<Module, CompileError> {
        for (i, stmt) in stmts.iter().enumerate() {
            let is_last = i == stmts.len() - 1;

            if is_last && matches!(stmt, Stmt::Expr(_)) {
                // Last statement is an expression - compile it to r0 for return
                if let Stmt::Expr(expr) = stmt {
                    // Use a temp register that won't clobber locals
                    let temp = self.next_temp_register().max(1);
                    self.compile_expr(expr, temp)?;
                    // Move to return register
                    self.emit(OpCode::Move);
                    self.emit_byte(0);
                    self.emit_byte(temp);
                    // Return that value
                    self.emit(OpCode::Return);
                    self.emit_byte(0);
                    self.emit_byte(0);
                }
            } else {
                self.compile_stmt(stmt)?;
            }
        }

        // If the last statement wasn't an expression, return nil
        if stmts.is_empty() || !matches!(stmts.last(), Some(Stmt::Expr(_))) {
            self.emit(OpCode::ReturnNil);
            self.emit_byte(0);
        }

        // Finalize main function
        let mut main = std::mem::replace(&mut self.function, Function::new("", 0));
        main.finalize();
        
        // Finalize all functions
        let mut functions = std::mem::take(&mut self.functions);
        for func in &mut functions {
            func.finalize();
        }
        
        Ok(Module {
            main,
            functions,
            strings: Vec::new(), // String interning happens at runtime
        })
    }

    pub(crate) fn finish(mut self) -> Function {
        self.function.num_locals = self.locals.len() as u8;
        self.function
    }

    // === Emission helpers ===

    pub(crate) fn emit(&mut self, op: OpCode) {
        self.function.chunk.emit(op, self.line);
    }

    pub(crate) fn emit_byte(&mut self, byte: u8) {
        self.function.chunk.emit_byte(byte, self.line);
    }

    pub(crate) fn emit_u16(&mut self, value: u16) {
        self.function.chunk.emit_u16(value, self.line);
    }

    pub(crate) fn emit_i32(&mut self, value: i32) {
        self.function.chunk.emit_i32(value, self.line);
    }

    pub(crate) fn emit_jump_placeholder(&mut self) -> usize {
        let offset = self.function.chunk.offset();
        self.function.chunk.emit_i16(0, self.line);
        offset
    }

    pub(crate) fn patch_jump(&mut self, offset: usize) {
        self.function.chunk.patch_jump(offset);
    }

    pub(crate) fn emit_loop(&mut self, loop_start: usize) {
        self.emit(OpCode::Loop);

        let offset = self.function.chunk.offset() - loop_start + 2;
        if offset > i16::MAX as usize {
            panic!("Loop too large");
        }

        let bytes = (offset as i16).to_le_bytes();
        self.emit_byte(bytes[0]);
        self.emit_byte(bytes[1]);
    }

    pub(crate) fn current_offset(&self) -> usize {
        self.function.chunk.offset()
    }

    // === Constants ===

    pub(crate) fn add_constant(&mut self, value: Value) -> Result<u16, CompileError> {
        let idx = self.function.chunk.add_constant(value);
        if idx > u16::MAX {
            return Err(CompileError::TooManyConstants);
        }
        Ok(idx)
    }

    pub(crate) fn add_name(&mut self, name: &str) -> Result<u16, CompileError> {
        self.add_constant(Value::String(name.to_string()))
    }

    pub(crate) fn add_function(&mut self, func: Function) -> Result<u16, CompileError> {
        // Add to root compiler's functions vector
        if let Some(enclosing_ptr) = self.enclosing {
            // Safety: enclosing pointer is valid during compilation
            let enclosing = unsafe { &mut *enclosing_ptr };
            enclosing.add_function(func)
        } else {
            self.functions.push(func);
            let idx = self.functions.len() - 1;
            if idx > u16::MAX as usize {
                return Err(CompileError::TooManyConstants);
            }
            Ok(idx as u16)
        }
    }

    // === Scope management ===

    pub(crate) fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    pub(crate) fn end_scope(&mut self) {
        self.scope_depth -= 1;

        // Pop locals that are out of scope
        while let Some(local) = self.locals.last() {
            if local.depth <= self.scope_depth {
                break;
            }
            self.locals.pop();
        }
    }

    // === Local variables ===

    pub(crate) fn declare_local(&mut self, name: &str) -> Result<u8, CompileError> {
        if self.locals.len() >= 256 {
            return Err(CompileError::TooManyLocals);
        }

        let slot = self.locals.len() as u8;
        self.locals.push(Local {
            name: name.to_string(),
            depth: self.scope_depth,
        });

        Ok(slot)
    }

    /// Get the next available temp register (above all locals)
    pub(crate) fn next_temp_register(&self) -> u8 {
        self.locals.len() as u8
    }

    pub(crate) fn resolve_local(&self, name: &str) -> Option<u8> {
        for (i, local) in self.locals.iter().enumerate().rev() {
            if local.name == name {
                return Some(i as u8);
            }
        }
        None
    }

    // === Upvalues ===

    pub(crate) fn resolve_upvalue(&mut self, name: &str) -> Result<Option<u8>, CompileError> {
        if self.enclosing.is_none() {
            return Ok(None);
        }

        // Safety: We control the lifetime of enclosing pointers
        let enclosing = unsafe { &mut *self.enclosing.unwrap() };

        // Check if it's a local in the enclosing function
        if let Some(local) = enclosing.resolve_local(name) {
            return Ok(Some(self.add_upvalue(local, true)?));
        }

        // Check if it's an upvalue in the enclosing function
        if let Some(upvalue) = enclosing.resolve_upvalue(name)? {
            return Ok(Some(self.add_upvalue(upvalue, false)?));
        }

        Ok(None)
    }

    fn add_upvalue(&mut self, index: u8, is_local: bool) -> Result<u8, CompileError> {
        // Check if we already have this upvalue
        for (i, uv) in self.function.upvalues.iter().enumerate() {
            if uv.index == index && uv.is_local == is_local {
                return Ok(i as u8);
            }
        }

        if self.function.upvalues.len() >= 256 {
            return Err(CompileError::TooManyUpvalues);
        }

        self.function.upvalues.push(UpvalueInfo { index, is_local });
        Ok((self.function.upvalues.len() - 1) as u8)
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn compile(source: &str) -> Result<Module, CompileError> {
        let lexer = Lexer::new(source);
        let mut parser = Parser::new(lexer);
        let stmts = parser.parse().unwrap();
        let mut compiler = Compiler::new();
        compiler.compile(&stmts)
    }

    #[test]
    fn test_disassemble() {
        let module = compile("let x = 1 + 2\nx").unwrap();
        let disasm = module.main.chunk.disassemble("main");
        assert!(disasm.contains("LoadInt"));
        assert!(disasm.contains("Add"));
    }
}
