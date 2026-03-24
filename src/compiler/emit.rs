//! Bytecode emission from AST

use super::{Compiler, CompileError};
use super::opcode::OpCode;
use crate::parser::{Expr, Stmt, BinaryOp, UnaryOp, FunctionDef};
use crate::vm::Value;

impl Compiler {
    /// Compile an expression into the given destination register
    pub fn compile_expr(&mut self, expr: &Expr, dst: u8) -> Result<(), CompileError> {
        match expr {
            Expr::Nil => {
                self.emit(OpCode::LoadNil);
                self.emit_byte(dst);
            }

            Expr::Bool(true) => {
                self.emit(OpCode::LoadTrue);
                self.emit_byte(dst);
            }

            Expr::Bool(false) => {
                self.emit(OpCode::LoadFalse);
                self.emit_byte(dst);
            }

            Expr::Int(n) => {
                if *n >= i32::MIN as i64 && *n <= i32::MAX as i64 {
                    self.emit(OpCode::LoadInt);
                    self.emit_byte(dst);
                    self.emit_i32(*n as i32);
                } else {
                    let idx = self.add_constant(Value::Int(*n))?;
                    self.emit(OpCode::LoadConst);
                    self.emit_byte(dst);
                    self.emit_u16(idx);
                }
            }

            Expr::Float(n) => {
                let idx = self.add_constant(Value::Float(*n))?;
                self.emit(OpCode::LoadConst);
                self.emit_byte(dst);
                self.emit_u16(idx);
            }

            Expr::String(s) => {
                let idx = self.add_constant(Value::String(s.clone()))?;
                self.emit(OpCode::LoadConst);
                self.emit_byte(dst);
                self.emit_u16(idx);
            }

            Expr::Identifier(name) => {
                if let Some(local) = self.resolve_local(name) {
                    self.emit(OpCode::GetLocal);
                    self.emit_byte(dst);
                    self.emit_byte(local);
                } else if let Some(upvalue) = self.resolve_upvalue(name)? {
                    self.emit(OpCode::GetUpvalue);
                    self.emit_byte(dst);
                    self.emit_byte(upvalue);
                } else {
                    let idx = self.add_name(name)?;
                    self.emit(OpCode::GetGlobal);
                    self.emit_byte(dst);
                    self.emit_u16(idx);
                }
            }

            Expr::Array(elements) => {
                // Compile elements into consecutive registers starting at dst+1
                for (i, elem) in elements.iter().enumerate() {
                    self.compile_expr(elem, dst + 1 + i as u8)?;
                }
                self.emit(OpCode::NewArray);
                self.emit_byte(dst);
                self.emit_byte(elements.len() as u8);
            }

            Expr::Table(pairs) => {
                self.emit(OpCode::NewTable);
                self.emit_byte(dst);

                for (key, value) in pairs {
                    self.compile_expr(key, dst + 1)?;
                    self.compile_expr(value, dst + 2)?;
                    self.emit(OpCode::TableSet);
                    self.emit_byte(dst);
                    self.emit_byte(dst + 1);
                    self.emit_byte(dst + 2);
                }
            }

            Expr::Binary(left, op, right) => {
                // Compile operands
                self.compile_expr(left, dst)?;
                self.compile_expr(right, dst + 1)?;

                let opcode = match op {
                    BinaryOp::Add => OpCode::Add,
                    BinaryOp::Sub => OpCode::Sub,
                    BinaryOp::Mul => OpCode::Mul,
                    BinaryOp::Div => OpCode::Div,
                    BinaryOp::Mod => OpCode::Mod,
                    BinaryOp::Pow => OpCode::Pow,
                    BinaryOp::Eq => OpCode::Eq,
                    BinaryOp::Ne => OpCode::Ne,
                    BinaryOp::Lt => OpCode::Lt,
                    BinaryOp::Le => OpCode::Le,
                    BinaryOp::Gt => OpCode::Gt,
                    BinaryOp::Ge => OpCode::Ge,
                    BinaryOp::And => OpCode::And,
                    BinaryOp::Or => OpCode::Or,
                    BinaryOp::BitAnd => OpCode::BitAnd,
                    BinaryOp::BitOr => OpCode::BitOr,
                    BinaryOp::BitXor => OpCode::BitXor,
                    BinaryOp::Shl => OpCode::Shl,
                    BinaryOp::Shr => OpCode::Shr,
                    BinaryOp::Concat => OpCode::Concat,
                    BinaryOp::Range | BinaryOp::RangeInclusive => {
                        // Ranges are handled specially at runtime
                        return Err(CompileError::NotImplemented("ranges".to_string()));
                    }
                };

                self.emit(opcode);
                self.emit_byte(dst);
                self.emit_byte(dst);
                self.emit_byte(dst + 1);
            }

            Expr::Unary(op, operand) => {
                self.compile_expr(operand, dst)?;

                let opcode = match op {
                    UnaryOp::Neg => OpCode::Neg,
                    UnaryOp::Not => OpCode::Not,
                    UnaryOp::BitNot => OpCode::BitNot,
                };

                self.emit(opcode);
                self.emit_byte(dst);
                self.emit_byte(dst);
            }

            Expr::Call(callee, args) => {
                // Compile callee into dst
                self.compile_expr(callee, dst)?;

                // Compile args into consecutive registers
                for (i, arg) in args.iter().enumerate() {
                    self.compile_expr(arg, dst + 1 + i as u8)?;
                }

                self.emit(OpCode::Call);
                self.emit_byte(dst);
                self.emit_byte(dst);
                self.emit_byte(args.len() as u8);
            }

            Expr::Index(array, index) => {
                self.compile_expr(array, dst)?;
                self.compile_expr(index, dst + 1)?;
                self.emit(OpCode::ArrayGet);
                self.emit_byte(dst);
                self.emit_byte(dst);
                self.emit_byte(dst + 1);
            }

            Expr::Field(object, field) => {
                self.compile_expr(object, dst)?;
                let idx = self.add_constant(Value::String(field.clone()))?;
                self.emit(OpCode::LoadConst);
                self.emit_byte(dst + 1);
                self.emit_u16(idx);
                self.emit(OpCode::TableGet);
                self.emit_byte(dst);
                self.emit_byte(dst);
                self.emit_byte(dst + 1);
            }

            Expr::If(cond, then_expr, else_expr) => {
                self.compile_expr(cond, dst)?;

                self.emit(OpCode::JumpIfNot);
                self.emit_byte(dst);
                let else_jump = self.emit_jump_placeholder();

                self.compile_expr(then_expr, dst)?;

                let end_jump = if else_expr.is_some() {
                    self.emit(OpCode::Jump);
                    Some(self.emit_jump_placeholder())
                } else {
                    None
                };

                self.patch_jump(else_jump);

                if let Some(else_expr) = else_expr {
                    self.compile_expr(else_expr, dst)?;
                    if let Some(end_jump) = end_jump {
                        self.patch_jump(end_jump);
                    }
                }
            }

            Expr::Block(stmts) => {
                self.begin_scope();
                for stmt in stmts {
                    self.compile_stmt(stmt)?;
                }
                self.end_scope();
                // Block evaluates to nil by default
                self.emit(OpCode::LoadNil);
                self.emit_byte(dst);
            }

            Expr::Function(def) => {
                self.compile_function(def, dst)?;
            }
        }

        Ok(())
    }

    /// Compile a statement
    pub fn compile_stmt(&mut self, stmt: &Stmt) -> Result<(), CompileError> {
        match stmt {
            Stmt::Expr(expr) => {
                // Expression statement - compile to temp register above locals
                let temp = self.next_temp_register();
                self.compile_expr(expr, temp)?;
                // Move result to r0 for potential return
                if temp != 0 {
                    self.emit(OpCode::Move);
                    self.emit_byte(0);
                    self.emit_byte(temp);
                }
            }

            Stmt::Let(name, _is_mut, initializer) => {
                let slot = self.declare_local(name)?;

                if let Some(init) = initializer {
                    self.compile_expr(init, slot)?;
                } else {
                    self.emit(OpCode::LoadNil);
                    self.emit_byte(slot);
                }
            }

            Stmt::Assign(target, value) => {
                match target {
                    Expr::Identifier(name) => {
                        if let Some(local) = self.resolve_local(name) {
                            self.compile_expr(value, local)?;
                        } else if let Some(upvalue) = self.resolve_upvalue(name)? {
                            self.compile_expr(value, 0)?;
                            self.emit(OpCode::SetUpvalue);
                            self.emit_byte(upvalue);
                            self.emit_byte(0);
                        } else {
                            self.compile_expr(value, 0)?;
                            let idx = self.add_name(name)?;
                            self.emit(OpCode::SetGlobal);
                            self.emit_u16(idx);
                            self.emit_byte(0);
                        }
                    }

                    Expr::Index(array, index) => {
                        self.compile_expr(array, 0)?;
                        self.compile_expr(index, 1)?;
                        self.compile_expr(value, 2)?;
                        self.emit(OpCode::ArraySet);
                        self.emit_byte(0);
                        self.emit_byte(1);
                        self.emit_byte(2);
                    }

                    Expr::Field(object, field) => {
                        self.compile_expr(object, 0)?;
                        let idx = self.add_constant(Value::String(field.clone()))?;
                        self.emit(OpCode::LoadConst);
                        self.emit_byte(1);
                        self.emit_u16(idx);
                        self.compile_expr(value, 2)?;
                        self.emit(OpCode::TableSet);
                        self.emit_byte(0);
                        self.emit_byte(1);
                        self.emit_byte(2);
                    }

                    _ => return Err(CompileError::InvalidAssignmentTarget),
                }
            }

            Stmt::If(cond, then_branch, else_branch) => {
                let temp = self.next_temp_register();
                self.compile_expr(cond, temp)?;

                self.emit(OpCode::JumpIfNot);
                self.emit_byte(temp);
                let else_jump = self.emit_jump_placeholder();

                for stmt in then_branch {
                    self.compile_stmt(stmt)?;
                }

                if let Some(else_stmts) = else_branch {
                    self.emit(OpCode::Jump);
                    let end_jump = self.emit_jump_placeholder();
                    self.patch_jump(else_jump);

                    for stmt in else_stmts {
                        self.compile_stmt(stmt)?;
                    }
                    self.patch_jump(end_jump);
                } else {
                    self.patch_jump(else_jump);
                }
            }

            Stmt::While(cond, body) => {
                let loop_start = self.current_offset();

                let temp = self.next_temp_register();
                self.compile_expr(cond, temp)?;

                self.emit(OpCode::JumpIfNot);
                self.emit_byte(temp);
                let exit_jump = self.emit_jump_placeholder();

                self.begin_scope();
                for stmt in body {
                    self.compile_stmt(stmt)?;
                }
                self.end_scope();

                self.emit_loop(loop_start);
                self.patch_jump(exit_jump);
            }

            Stmt::For(name, iterable, body) => {
                // for x in iter { body }
                // Compiled as:
                //   iter_val = iterable
                //   iter = get_iter(iter_val)
                // loop:
                //   x = iter_next(iter) or jump to end
                //   body
                //   jump loop
                // end:

                self.begin_scope();

                // Compile iterable and get iterator
                self.compile_expr(iterable, 0)?;
                self.emit(OpCode::GetIter);
                self.emit_byte(1);
                self.emit_byte(0);

                let loop_start = self.current_offset();

                // Declare loop variable
                let var_slot = self.declare_local(name)?;

                // Get next value or jump to end
                self.emit(OpCode::IterNext);
                self.emit_byte(var_slot);
                self.emit_byte(1);
                let exit_jump = self.emit_jump_placeholder();

                // Compile body
                for stmt in body {
                    self.compile_stmt(stmt)?;
                }

                self.emit_loop(loop_start);
                self.patch_jump(exit_jump);

                self.end_scope();
            }

            Stmt::Return(value) => {
                if let Some(expr) = value {
                    self.compile_expr(expr, 0)?;
                    self.emit(OpCode::Return);
                    self.emit_byte(0);
                    self.emit_byte(0);
                } else {
                    self.emit(OpCode::ReturnNil);
                    self.emit_byte(0);
                }
            }

            Stmt::Function(def) => {
                // Named function becomes a local or global
                let name = def.name.as_ref().ok_or(CompileError::InvalidAssignmentTarget)?;

                if self.scope_depth > 0 {
                    let slot = self.declare_local(name)?;
                    self.compile_function(def, slot)?;
                } else {
                    self.compile_function(def, 0)?;
                    let idx = self.add_name(name)?;
                    self.emit(OpCode::SetGlobal);
                    self.emit_u16(idx);
                    self.emit_byte(0);
                }
            }
        }

        Ok(())
    }

    /// Compile a function definition
    fn compile_function(&mut self, def: &FunctionDef, dst: u8) -> Result<(), CompileError> {
        // Create a new compiler for the function
        let mut func_compiler = Compiler::new_function(
            def.name.clone().unwrap_or_else(|| "<anon>".to_string()),
            def.params.len() as u8,
        );

        // Set parent for upvalue resolution
        func_compiler.enclosing = Some(self as *mut Compiler);

        // Declare parameters as locals
        func_compiler.begin_scope();
        for param in &def.params {
            func_compiler.declare_local(param)?;
        }

        // Compile body
        for stmt in &def.body {
            func_compiler.compile_stmt(stmt)?;
        }

        // Implicit return nil
        func_compiler.emit(OpCode::ReturnNil);
        func_compiler.emit_byte(0);

        func_compiler.end_scope();

        // Add function to constants and emit closure
        let func = func_compiler.finish();
        let func_idx = self.add_function(func)?;

        self.emit(OpCode::Closure);
        self.emit_byte(dst);
        self.emit_u16(func_idx);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn compile(source: &str) -> Result<super::super::Module, CompileError> {
        let lexer = Lexer::new(source);
        let mut parser = Parser::new(lexer);
        let stmts = parser.parse().unwrap();
        let mut compiler = Compiler::new();
        compiler.compile(&stmts)
    }

    #[test]
    fn test_compile_int() {
        let module = compile("42").unwrap();
        assert!(module.main.chunk.code.len() > 0);
    }

    #[test]
    fn test_compile_add() {
        let module = compile("1 + 2").unwrap();
        assert!(module.main.chunk.code.len() > 0);
    }

    #[test]
    fn test_compile_let() {
        let module = compile("let x = 42").unwrap();
        assert!(module.main.chunk.code.len() > 0);
    }

    #[test]
    fn test_compile_function() {
        let module = compile("fn add(a, b) { return a + b }").unwrap();
        assert_eq!(module.functions.len(), 1);
    }

    #[test]
    fn test_compile_if() {
        let module = compile("if true { 1 } else { 2 }").unwrap();
        assert!(module.main.chunk.code.len() > 0);
    }

    #[test]
    fn test_compile_while() {
        let module = compile("let x = 0\nwhile x < 10 { x = x + 1 }").unwrap();
        assert!(module.main.chunk.code.len() > 0);
    }
}
