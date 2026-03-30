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
                    BinaryOp::Range => {
                        // Emit range creation
                        self.emit(OpCode::MakeRange);
                        self.emit_byte(dst);
                        self.emit_byte(dst);
                        self.emit_byte(dst + 1);
                        return Ok(());
                    }
                    BinaryOp::RangeInclusive => {
                        // Emit inclusive range creation
                        self.emit(OpCode::MakeRangeIncl);
                        self.emit_byte(dst);
                        self.emit_byte(dst);
                        self.emit_byte(dst + 1);
                        return Ok(());
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
                // Check for method call: obj.method(args)
                if let Expr::Field(obj, method_name) = callee.as_ref() {
                    // Compile object
                    self.compile_expr(obj, dst)?;
                    
                    // Load method name as constant
                    let name_idx = self.add_constant(Value::String(method_name.clone()))?;
                    self.emit(OpCode::LoadConst);
                    self.emit_byte(dst + 1);
                    self.emit_u16(name_idx);
                    
                    // Compile args
                    for (i, arg) in args.iter().enumerate() {
                        self.compile_expr(arg, dst + 2 + i as u8)?;
                    }
                    
                    // MethodCall: dst = obj.method(args)
                    // At runtime: if obj is table with method -> call that
                    //             else -> call global method(obj, args)
                    self.emit(OpCode::MethodCall);
                    self.emit_byte(dst);        // result dest
                    self.emit_byte(dst);        // object reg
                    self.emit_byte(dst + 1);    // method name reg
                    self.emit_byte(args.len() as u8);  // argc
                } else {
                    // Regular function call
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
                if stmts.is_empty() {
                    self.emit(OpCode::LoadNil);
                    self.emit_byte(dst);
                } else {
                    // Compile all but last statement normally
                    for stmt in &stmts[..stmts.len() - 1] {
                        self.compile_stmt(stmt)?;
                    }
                    // Handle last statement based on type
                    match stmts.last() {
                        Some(crate::parser::Stmt::Expr(last_expr)) => {
                            // Expression - compile to dst for implicit return
                            self.compile_expr(last_expr, dst)?;
                        }
                        Some(crate::parser::Stmt::Return(_)) => {
                            // Return statement - compile it, no need for LoadNil
                            // (control flow exits the function)
                            if let Some(last_stmt) = stmts.last() {
                                self.compile_stmt(last_stmt)?;
                            }
                        }
                        Some(last_stmt) => {
                            // Other statement - compile and load nil as block value
                            self.compile_stmt(last_stmt)?;
                            self.emit(OpCode::LoadNil);
                            self.emit_byte(dst);
                        }
                        None => {
                            self.emit(OpCode::LoadNil);
                            self.emit_byte(dst);
                        }
                    }
                }
                self.end_scope();
            }

            Expr::Function(def) => {
                self.compile_function(def, dst)?;
            }

            Expr::Match(value, arms) => {
                // Compile the value to match against
                self.compile_expr(value, dst)?;
                
                let mut end_jumps = Vec::new();
                
                for arm in arms {
                    // For each arm, check if pattern matches
                    match &arm.pattern {
                        crate::parser::Pattern::Wildcard => {
                            // Wildcard always matches - compile body directly
                            self.compile_expr(&arm.body, dst)?;
                            // Jump to end (this is the last arm effectively)
                            self.emit(OpCode::Jump);
                            end_jumps.push(self.emit_jump_placeholder());
                        }
                        crate::parser::Pattern::Literal(lit_expr) => {
                            // Compare value with literal
                            self.compile_expr(lit_expr, dst + 1)?;
                            self.emit(OpCode::Eq);
                            self.emit_byte(dst + 2);
                            self.emit_byte(dst);
                            self.emit_byte(dst + 1);
                            
                            // Jump if not equal
                            self.emit(OpCode::JumpIfNot);
                            self.emit_byte(dst + 2);
                            let next_arm = self.emit_jump_placeholder();
                            
                            // Check guard if present
                            if let Some(guard) = &arm.guard {
                                self.compile_expr(guard, dst + 2)?;
                                self.emit(OpCode::JumpIfNot);
                                self.emit_byte(dst + 2);
                                let guard_fail = self.emit_jump_placeholder();
                                
                                // Pattern and guard matched - compile body
                                self.compile_expr(&arm.body, dst)?;
                                self.emit(OpCode::Jump);
                                end_jumps.push(self.emit_jump_placeholder());
                                
                                self.patch_jump(guard_fail);
                            } else {
                                // Pattern matched - compile body
                                self.compile_expr(&arm.body, dst)?;
                                self.emit(OpCode::Jump);
                                end_jumps.push(self.emit_jump_placeholder());
                            }
                            
                            self.patch_jump(next_arm);
                        }
                        crate::parser::Pattern::Binding(name) => {
                            // Binding pattern - bind value to name and execute body
                            self.begin_scope();
                            let slot = self.declare_local(name)?;
                            self.emit(OpCode::Move);
                            self.emit_byte(slot);
                            self.emit_byte(dst);
                            
                            // Check guard if present
                            if let Some(guard) = &arm.guard {
                                self.compile_expr(guard, dst + 1)?;
                                self.emit(OpCode::JumpIfNot);
                                self.emit_byte(dst + 1);
                                let guard_fail = self.emit_jump_placeholder();
                                
                                self.compile_expr(&arm.body, dst)?;
                                self.end_scope();
                                self.emit(OpCode::Jump);
                                end_jumps.push(self.emit_jump_placeholder());
                                
                                self.patch_jump(guard_fail);
                            } else {
                                self.compile_expr(&arm.body, dst)?;
                                self.end_scope();
                                self.emit(OpCode::Jump);
                                end_jumps.push(self.emit_jump_placeholder());
                            }
                        }
                        crate::parser::Pattern::Range(start, end, inclusive) => {
                            // Range pattern: start <= value && value < end (or <= if inclusive)
                            self.compile_expr(start, dst + 1)?;
                            self.compile_expr(end, dst + 2)?;
                            
                            // Check value >= start
                            self.emit(OpCode::Ge);
                            self.emit_byte(dst + 3);
                            self.emit_byte(dst);
                            self.emit_byte(dst + 1);
                            
                            self.emit(OpCode::JumpIfNot);
                            self.emit_byte(dst + 3);
                            let next_arm = self.emit_jump_placeholder();
                            
                            // Check value < end (or <= if inclusive)
                            if *inclusive {
                                self.emit(OpCode::Le);
                            } else {
                                self.emit(OpCode::Lt);
                            }
                            self.emit_byte(dst + 3);
                            self.emit_byte(dst);
                            self.emit_byte(dst + 2);
                            
                            self.emit(OpCode::JumpIfNot);
                            self.emit_byte(dst + 3);
                            let range_fail = self.emit_jump_placeholder();
                            
                            // Pattern matched - compile body
                            self.compile_expr(&arm.body, dst)?;
                            self.emit(OpCode::Jump);
                            end_jumps.push(self.emit_jump_placeholder());
                            
                            self.patch_jump(next_arm);
                            self.patch_jump(range_fail);
                        }
                    }
                }
                
                // If no pattern matched, result is nil
                self.emit(OpCode::LoadNil);
                self.emit_byte(dst);
                
                // Patch all end jumps
                for jump in end_jumps {
                    self.patch_jump(jump);
                }
            }

            Expr::Try(inner) => {
                // Try expression - wraps result in a Result-like table
                // { ok: true, value: result } or { ok: false, err: error_msg }
                // For now, we just compile the inner expression and wrap success
                // Real error handling would need VM support for catch/throw
                
                // Compile inner expression
                self.compile_expr(inner, dst)?;
                
                // Create result table { ok: true, value: <result> }
                self.emit(OpCode::NewTable);
                self.emit_byte(dst + 1);
                
                // Set ok = true
                let ok_idx = self.add_constant(Value::String("ok".to_string()))?;
                self.emit(OpCode::LoadConst);
                self.emit_byte(dst + 2);
                self.emit_u16(ok_idx);
                self.emit(OpCode::LoadTrue);
                self.emit_byte(dst + 3);
                self.emit(OpCode::TableSet);
                self.emit_byte(dst + 1);
                self.emit_byte(dst + 2);
                self.emit_byte(dst + 3);
                
                // Set value = result
                let val_idx = self.add_constant(Value::String("value".to_string()))?;
                self.emit(OpCode::LoadConst);
                self.emit_byte(dst + 2);
                self.emit_u16(val_idx);
                self.emit(OpCode::TableSet);
                self.emit_byte(dst + 1);
                self.emit_byte(dst + 2);
                self.emit_byte(dst);
                
                // Move result table to dst
                self.emit(OpCode::Move);
                self.emit_byte(dst);
                self.emit_byte(dst + 1);
            }

            Expr::Interpolation(parts) => {
                // String interpolation - concatenate all parts
                if parts.is_empty() {
                    let idx = self.add_constant(Value::String(String::new()))?;
                    self.emit(OpCode::LoadConst);
                    self.emit_byte(dst);
                    self.emit_u16(idx);
                } else {
                    // Compile first part
                    match &parts[0] {
                        crate::parser::InterpolationPart::Literal(s) => {
                            let idx = self.add_constant(Value::String(s.clone()))?;
                            self.emit(OpCode::LoadConst);
                            self.emit_byte(dst);
                            self.emit_u16(idx);
                        }
                        crate::parser::InterpolationPart::Expr(e) => {
                            // Convert to string using str(e)
                            // Call convention: func at dst, args at dst+1, dst+2, ...
                            let str_idx = self.add_name("str")?;
                            self.emit(OpCode::GetGlobal);
                            self.emit_byte(dst);
                            self.emit_u16(str_idx);
                            self.compile_expr(e, dst + 1)?;
                            self.emit(OpCode::Call);
                            self.emit_byte(dst);
                            self.emit_byte(dst);
                            self.emit_byte(1);
                        }
                    }
                    
                    // Concatenate remaining parts
                    for part in parts.iter().skip(1) {
                        match part {
                            crate::parser::InterpolationPart::Literal(s) => {
                                let idx = self.add_constant(Value::String(s.clone()))?;
                                self.emit(OpCode::LoadConst);
                                self.emit_byte(dst + 1);
                                self.emit_u16(idx);
                            }
                            crate::parser::InterpolationPart::Expr(e) => {
                                // Convert to string using str(e)
                                // Use dst+1 as temp call location
                                let str_idx = self.add_name("str")?;
                                self.emit(OpCode::GetGlobal);
                                self.emit_byte(dst + 1);
                                self.emit_u16(str_idx);
                                self.compile_expr(e, dst + 2)?;
                                self.emit(OpCode::Call);
                                self.emit_byte(dst + 1);
                                self.emit_byte(dst + 1);
                                self.emit_byte(1);
                            }
                        }
                        
                        // Concatenate
                        self.emit(OpCode::Concat);
                        self.emit_byte(dst);
                        self.emit_byte(dst);
                        self.emit_byte(dst + 1);
                    }
                }
            }
        }

        Ok(())
    }

    /// Compile a statement
    pub fn compile_stmt(&mut self, stmt: &Stmt) -> Result<(), CompileError> {
        match stmt {
            Stmt::Expr(expr) => {
                // Expression statement - compile to temp register above locals
                // Don't move to r0 here - that clobbers locals. The compiler's
                // compile() method handles returning the last expression value.
                let temp = self.next_temp_register();
                self.compile_expr(expr, temp)?;
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
                //   $iter = get_iter(iterable)  (hidden local)
                // loop_start:
                //   (x, $done) = iter_next($iter)
                //   jump_if $done end
                //   body
                //   jump loop_start
                // end:

                self.begin_scope();

                // Declare hidden iterator local (use a name that can't conflict)
                let iter_slot = self.declare_local(&format!("$iter_{}", self.locals.len()))?;
                // Declare hidden done flag local
                let done_slot = self.declare_local(&format!("$done_{}", self.locals.len()))?;
                // Declare loop variable
                let var_slot = self.declare_local(name)?;

                // Compile iterable to temp, then get iterator into iter_slot
                let temp = self.next_temp_register();
                self.compile_expr(iterable, temp)?;
                self.emit(OpCode::GetIter);
                self.emit_byte(iter_slot);
                self.emit_byte(temp);

                let loop_start = self.current_offset();

                // Get next value, sets done flag
                self.emit(OpCode::IterNext);
                self.emit_byte(var_slot);
                self.emit_byte(done_slot);
                self.emit_byte(iter_slot);

                // Jump to end if done
                self.emit(OpCode::JumpIf);
                self.emit_byte(done_slot);
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
                    // Compile to temp register to avoid clobbering locals
                    let temp = self.next_temp_register();
                    self.compile_expr(expr, temp)?;
                    self.emit(OpCode::Return);
                    self.emit_byte(0);
                    self.emit_byte(temp);
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
        // Note: nested function definitions added their functions to our functions vec via enclosing
        let func = func_compiler.finish();
        let num_upvalues = func.upvalues.len();
        let upvalue_info: Vec<_> = func.upvalues.iter().cloned().collect();
        let func_idx = self.add_function(func)?;

        self.emit(OpCode::Closure);
        self.emit_byte(dst);
        self.emit_u16(func_idx);
        
        // Emit upvalue info (is_local, index for each upvalue)
        self.emit_byte(num_upvalues as u8);
        for uv in upvalue_info {
            self.emit_byte(if uv.is_local { 1 } else { 0 });
            self.emit_byte(uv.index);
        }

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
