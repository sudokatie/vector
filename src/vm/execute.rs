//! Bytecode execution engine

use super::{VM, RuntimeError};
use super::value::Value;
use super::frame::CallFrame;
use crate::compiler::{OpCode, Module};
use crate::jit::TypeTag;
use std::rc::Rc;

impl VM {
    /// Run a compiled module
    pub fn run(&mut self, module: Module) -> Result<Value, RuntimeError> {
        // Set up main function
        let main = Rc::new(module.main);
        self.functions = module.functions.into_iter().map(Rc::new).collect();

        // Initialize JIT profiling for all functions
        if let Some(jit) = &mut self.jit {
            for (i, func) in self.functions.iter().enumerate() {
                jit.init_function(i, func.arity);
            }
        }

        let frame = CallFrame::new(main, 0);
        self.frames.push(frame);

        self.execute()
    }

    /// Record a function call for JIT profiling
    fn profile_call(&mut self, func_idx: usize) {
        if let Some(jit) = &mut self.jit {
            jit.record_call(func_idx);
        }
    }

    /// Record a loop iteration for JIT profiling
    fn profile_loop(&mut self, loop_offset: usize) {
        if let Some(jit) = &mut self.jit {
            // Use current function index
            let func_idx = self.frames.len().saturating_sub(1);
            jit.record_loop(func_idx, loop_offset);
        }
    }

    /// Get type tag for a value (for profiling)
    fn value_type_tag(value: &Value) -> TypeTag {
        match value {
            Value::Nil => TypeTag::Nil,
            Value::Bool(_) => TypeTag::Bool,
            Value::Int(_) => TypeTag::Int,
            Value::Float(_) => TypeTag::Float,
            Value::String(_) => TypeTag::String,
            Value::Array(_) => TypeTag::Array,
            Value::Table(_) => TypeTag::Table,
            Value::Function(_) | Value::NativeFunction(_) => TypeTag::Function,
        }
    }

    /// Main execution loop
    fn execute(&mut self) -> Result<Value, RuntimeError> {
        loop {
            let op = self.read_opcode()?;

            match op {
                OpCode::LoadNil => {
                    let dst = self.read_byte()?;
                    self.set_register(dst, Value::Nil);
                }

                OpCode::LoadTrue => {
                    let dst = self.read_byte()?;
                    self.set_register(dst, Value::Bool(true));
                }

                OpCode::LoadFalse => {
                    let dst = self.read_byte()?;
                    self.set_register(dst, Value::Bool(false));
                }

                OpCode::LoadInt => {
                    let dst = self.read_byte()?;
                    let value = self.read_i32()?;
                    self.set_register(dst, Value::Int(value as i64));
                }

                OpCode::LoadConst => {
                    let dst = self.read_byte()?;
                    let idx = self.read_u16()?;
                    let value = self.get_constant(idx)?;
                    self.set_register(dst, value);
                }

                OpCode::Move => {
                    let dst = self.read_byte()?;
                    let src = self.read_byte()?;
                    let value = self.get_register(src).clone();
                    self.set_register(dst, value);
                }

                // Arithmetic
                OpCode::Add => self.binary_op(|a, b| {
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => Ok(Value::Int(*x + *y)),
                        (Value::Float(x), Value::Float(y)) => Ok(Value::Float(*x + *y)),
                        (Value::Int(x), Value::Float(y)) => Ok(Value::Float(*x as f64 + *y)),
                        (Value::Float(x), Value::Int(y)) => Ok(Value::Float(*x + *y as f64)),
                        _ => Err(RuntimeError::TypeError {
                            expected: "number".to_string(),
                            got: a.type_name().to_string(),
                        }),
                    }
                })?,

                OpCode::Sub => self.binary_op(|a, b| {
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => Ok(Value::Int(*x - *y)),
                        (Value::Float(x), Value::Float(y)) => Ok(Value::Float(*x - *y)),
                        (Value::Int(x), Value::Float(y)) => Ok(Value::Float(*x as f64 - *y)),
                        (Value::Float(x), Value::Int(y)) => Ok(Value::Float(*x - *y as f64)),
                        _ => Err(RuntimeError::TypeError {
                            expected: "number".to_string(),
                            got: a.type_name().to_string(),
                        }),
                    }
                })?,

                OpCode::Mul => self.binary_op(|a, b| {
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => Ok(Value::Int(*x * *y)),
                        (Value::Float(x), Value::Float(y)) => Ok(Value::Float(*x * *y)),
                        (Value::Int(x), Value::Float(y)) => Ok(Value::Float(*x as f64 * *y)),
                        (Value::Float(x), Value::Int(y)) => Ok(Value::Float(*x * *y as f64)),
                        _ => Err(RuntimeError::TypeError {
                            expected: "number".to_string(),
                            got: a.type_name().to_string(),
                        }),
                    }
                })?,

                OpCode::Div => self.binary_op(|a, b| {
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => {
                            if *y == 0 {
                                return Err(RuntimeError::DivisionByZero);
                            }
                            Ok(Value::Int(*x / *y))
                        }
                        (Value::Float(x), Value::Float(y)) => Ok(Value::Float(*x / *y)),
                        (Value::Int(x), Value::Float(y)) => Ok(Value::Float(*x as f64 / *y)),
                        (Value::Float(x), Value::Int(y)) => Ok(Value::Float(*x / *y as f64)),
                        _ => Err(RuntimeError::TypeError {
                            expected: "number".to_string(),
                            got: a.type_name().to_string(),
                        }),
                    }
                })?,

                OpCode::Mod => self.binary_op(|a, b| {
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => {
                            if *y == 0 {
                                return Err(RuntimeError::DivisionByZero);
                            }
                            Ok(Value::Int(*x % *y))
                        }
                        (Value::Float(x), Value::Float(y)) => Ok(Value::Float(*x % *y)),
                        (Value::Int(x), Value::Float(y)) => Ok(Value::Float(*x as f64 % *y)),
                        (Value::Float(x), Value::Int(y)) => Ok(Value::Float(*x % *y as f64)),
                        _ => Err(RuntimeError::TypeError {
                            expected: "number".to_string(),
                            got: a.type_name().to_string(),
                        }),
                    }
                })?,

                OpCode::Pow => self.binary_op(|a, b| {
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) if *y >= 0 => {
                            Ok(Value::Int(x.pow(*y as u32)))
                        }
                        (Value::Int(x), Value::Int(y)) => {
                            Ok(Value::Float((*x as f64).powf(*y as f64)))
                        }
                        (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x.powf(*y))),
                        (Value::Int(x), Value::Float(y)) => Ok(Value::Float((*x as f64).powf(*y))),
                        (Value::Float(x), Value::Int(y)) => Ok(Value::Float(x.powf(*y as f64))),
                        _ => Err(RuntimeError::TypeError {
                            expected: "number".to_string(),
                            got: a.type_name().to_string(),
                        }),
                    }
                })?,

                OpCode::Neg => {
                    let dst = self.read_byte()?;
                    let src = self.read_byte()?;
                    let value = self.get_register(src).clone();
                    let result = match value {
                        Value::Int(x) => Value::Int(-x),
                        Value::Float(x) => Value::Float(-x),
                        _ => return Err(RuntimeError::TypeError {
                            expected: "number".to_string(),
                            got: value.type_name().to_string(),
                        }),
                    };
                    self.set_register(dst, result);
                }

                // Comparison
                OpCode::Eq => self.binary_op(|a, b| Ok(Value::Bool(a == b)))?,
                OpCode::Ne => self.binary_op(|a, b| Ok(Value::Bool(a != b)))?,

                OpCode::Lt => self.comparison_op(|a, b| a < b)?,
                OpCode::Le => self.comparison_op(|a, b| a <= b)?,
                OpCode::Gt => self.comparison_op(|a, b| a > b)?,
                OpCode::Ge => self.comparison_op(|a, b| a >= b)?,

                // Logical
                OpCode::And => {
                    let dst = self.read_byte()?;
                    let left = self.read_byte()?;
                    let right = self.read_byte()?;
                    let left_val = self.get_register(left).clone();
                    let result = if !left_val.is_truthy() {
                        left_val
                    } else {
                        self.get_register(right).clone()
                    };
                    self.set_register(dst, result);
                }

                OpCode::Or => {
                    let dst = self.read_byte()?;
                    let left = self.read_byte()?;
                    let right = self.read_byte()?;
                    let left_val = self.get_register(left).clone();
                    let result = if left_val.is_truthy() {
                        left_val
                    } else {
                        self.get_register(right).clone()
                    };
                    self.set_register(dst, result);
                }

                OpCode::Not => {
                    let dst = self.read_byte()?;
                    let src = self.read_byte()?;
                    let value = self.get_register(src);
                    self.set_register(dst, Value::Bool(!value.is_truthy()));
                }

                // Bitwise
                OpCode::BitAnd => self.bitwise_op(|a, b| a & b)?,
                OpCode::BitOr => self.bitwise_op(|a, b| a | b)?,
                OpCode::BitXor => self.bitwise_op(|a, b| a ^ b)?,
                OpCode::Shl => self.bitwise_op(|a, b| a << b)?,
                OpCode::Shr => self.bitwise_op(|a, b| a >> b)?,

                OpCode::BitNot => {
                    let dst = self.read_byte()?;
                    let src = self.read_byte()?;
                    let value = self.get_register(src);
                    let result = match value {
                        Value::Int(x) => Value::Int(!x),
                        _ => return Err(RuntimeError::TypeError {
                            expected: "int".to_string(),
                            got: value.type_name().to_string(),
                        }),
                    };
                    self.set_register(dst, result);
                }

                // String
                OpCode::Concat => self.binary_op(|a, b| {
                    match (a, b) {
                        (Value::String(x), Value::String(y)) => {
                            Ok(Value::String(format!("{}{}", x, y)))
                        }
                        _ => Err(RuntimeError::TypeError {
                            expected: "string".to_string(),
                            got: a.type_name().to_string(),
                        }),
                    }
                })?,

                // Control flow
                OpCode::Jump => {
                    let offset = self.read_i16()?;
                    self.frame_mut().ip = (self.frame().ip as i32 + offset as i32) as usize;
                }

                OpCode::JumpIf => {
                    let cond = self.read_byte()?;
                    let offset = self.read_i16()?;
                    if self.get_register(cond).is_truthy() {
                        self.frame_mut().ip = (self.frame().ip as i32 + offset as i32) as usize;
                    }
                }

                OpCode::JumpIfNot => {
                    let cond = self.read_byte()?;
                    let offset = self.read_i16()?;
                    if !self.get_register(cond).is_truthy() {
                        self.frame_mut().ip = (self.frame().ip as i32 + offset as i32) as usize;
                    }
                }

                OpCode::Loop => {
                    let offset = self.read_u16()?;
                    let loop_start = self.frame().ip - offset as usize;
                    self.profile_loop(loop_start);
                    self.frame_mut().ip -= offset as usize;
                }

                // Functions
                OpCode::Call => {
                    let dst = self.read_byte()?;
                    let func_reg = self.read_byte()?;
                    let argc = self.read_byte()?;

                    let callee = self.get_register(func_reg).clone();
                    match callee {
                        Value::Function(func_idx) => {
                            // Profile the call
                            self.profile_call(func_idx as usize);

                            let func = self.functions[func_idx as usize].clone();
                            if func.arity != argc {
                                return Err(RuntimeError::ArityMismatch {
                                    expected: func.arity,
                                    got: argc,
                                });
                            }

                            // Copy arguments to new frame
                            let mut new_frame = CallFrame::new_with_return(func, self.frames.len(), dst);

                            for i in 0..argc {
                                let arg = self.get_register(dst + 1 + i).clone();
                                new_frame.set_register(i, arg);
                            }

                            self.frames.push(new_frame);
                        }
                        Value::NativeFunction(native_fn) => {
                            let mut args = Vec::with_capacity(argc as usize);
                            for i in 0..argc {
                                args.push(self.get_register(dst + 1 + i).clone());
                            }
                            let result = native_fn(&args)?;
                            self.set_register(dst, result);
                        }
                        _ => return Err(RuntimeError::NotCallable(callee.type_name().to_string())),
                    }
                }

                OpCode::Return => {
                    let _dst = self.read_byte()?;
                    let src = self.read_byte()?;
                    let result = self.get_register(src).clone();

                    // Get return register before popping
                    let return_reg = self.frame().return_register;
                    self.frames.pop();

                    if self.frames.is_empty() {
                        return Ok(result);
                    }

                    self.set_register(return_reg, result);
                }

                OpCode::ReturnNil => {
                    let _dst = self.read_byte()?;

                    // Get return register before popping
                    let return_reg = self.frame().return_register;
                    self.frames.pop();

                    if self.frames.is_empty() {
                        return Ok(Value::Nil);
                    }

                    self.set_register(return_reg, Value::Nil);
                }

                OpCode::TailCall => {
                    // For now, implement as regular call
                    // TODO: Proper tail call optimization
                    let func_reg = self.read_byte()?;
                    let argc = self.read_byte()?;

                    let callee = self.get_register(func_reg).clone();
                    match callee {
                        Value::Function(func_idx) => {
                            let func = self.functions[func_idx as usize].clone();
                            let mut new_frame = CallFrame::new(func, self.frames.len());

                            for i in 0..argc {
                                let arg = self.get_register(1 + i).clone();
                                new_frame.set_register(i, arg);
                            }

                            // Replace current frame
                            self.frames.pop();
                            self.frames.push(new_frame);
                        }
                        _ => return Err(RuntimeError::NotCallable(callee.type_name().to_string())),
                    }
                }

                // Variables
                OpCode::GetGlobal => {
                    let dst = self.read_byte()?;
                    let idx = self.read_u16()?;
                    let name = self.get_constant(idx)?;
                    if let Value::String(name) = name {
                        let value = self.globals.get(&name)
                            .cloned()
                            .unwrap_or(Value::Nil);
                        self.set_register(dst, value);
                    }
                }

                OpCode::SetGlobal => {
                    let idx = self.read_u16()?;
                    let src = self.read_byte()?;
                    let name = self.get_constant(idx)?;
                    if let Value::String(name) = name {
                        let value = self.get_register(src).clone();
                        self.globals.insert(name, value);
                    }
                }

                OpCode::GetLocal => {
                    let dst = self.read_byte()?;
                    let slot = self.read_byte()?;
                    let value = self.get_register(slot).clone();
                    self.set_register(dst, value);
                }

                OpCode::SetLocal => {
                    let slot = self.read_byte()?;
                    let src = self.read_byte()?;
                    let value = self.get_register(src).clone();
                    self.set_register(slot, value);
                }

                // Collections
                OpCode::NewArray => {
                    let dst = self.read_byte()?;
                    let count = self.read_byte()?;
                    let mut elements = Vec::with_capacity(count as usize);
                    for i in 0..count {
                        elements.push(self.get_register(dst + 1 + i).clone());
                    }
                    self.set_register(dst, Value::Array(Rc::new(std::cell::RefCell::new(elements))));
                }

                OpCode::ArrayGet => {
                    let dst = self.read_byte()?;
                    let arr = self.read_byte()?;
                    let idx = self.read_byte()?;

                    let array = self.get_register(arr).clone();
                    let index = self.get_register(idx).clone();

                    match (&array, &index) {
                        (Value::Array(arr), Value::Int(i)) => {
                            let arr = arr.borrow();
                            let idx = *i as usize;
                            if idx >= arr.len() {
                                return Err(RuntimeError::IndexOutOfBounds(*i));
                            }
                            self.set_register(dst, arr[idx].clone());
                        }
                        _ => return Err(RuntimeError::TypeError {
                            expected: "array[int]".to_string(),
                            got: format!("{}[{}]", array.type_name(), index.type_name()),
                        }),
                    }
                }

                OpCode::ArraySet => {
                    let arr = self.read_byte()?;
                    let idx = self.read_byte()?;
                    let val = self.read_byte()?;

                    let array = self.get_register(arr).clone();
                    let index = self.get_register(idx).clone();
                    let value = self.get_register(val).clone();

                    match (&array, &index) {
                        (Value::Array(arr), Value::Int(i)) => {
                            let mut arr = arr.borrow_mut();
                            let idx = *i as usize;
                            if idx >= arr.len() {
                                return Err(RuntimeError::IndexOutOfBounds(*i));
                            }
                            arr[idx] = value;
                        }
                        _ => return Err(RuntimeError::TypeError {
                            expected: "array[int]".to_string(),
                            got: format!("{}[{}]", array.type_name(), index.type_name()),
                        }),
                    }
                }

                OpCode::NewTable => {
                    let dst = self.read_byte()?;
                    self.set_register(dst, Value::Table(Rc::new(std::cell::RefCell::new(
                        fnv::FnvHashMap::default()
                    ))));
                }

                OpCode::TableGet => {
                    let dst = self.read_byte()?;
                    let tbl = self.read_byte()?;
                    let key = self.read_byte()?;

                    let table = self.get_register(tbl).clone();
                    let key_val = self.get_register(key).clone();

                    match &table {
                        Value::Table(tbl) => {
                            let tbl = tbl.borrow();
                            let value = tbl.get(&key_val).cloned().unwrap_or(Value::Nil);
                            self.set_register(dst, value);
                        }
                        _ => return Err(RuntimeError::TypeError {
                            expected: "table".to_string(),
                            got: table.type_name().to_string(),
                        }),
                    }
                }

                OpCode::TableSet => {
                    let tbl = self.read_byte()?;
                    let key = self.read_byte()?;
                    let val = self.read_byte()?;

                    let table = self.get_register(tbl).clone();
                    let key_val = self.get_register(key).clone();
                    let value = self.get_register(val).clone();

                    match &table {
                        Value::Table(tbl) => {
                            let mut tbl = tbl.borrow_mut();
                            tbl.insert(key_val, value);
                        }
                        _ => return Err(RuntimeError::TypeError {
                            expected: "table".to_string(),
                            got: table.type_name().to_string(),
                        }),
                    }
                }

                OpCode::Closure => {
                    let dst = self.read_byte()?;
                    let func_idx = self.read_u16()?;
                    self.set_register(dst, Value::Function(func_idx));
                }

                OpCode::GetUpvalue | OpCode::SetUpvalue | OpCode::CloseUpvalue => {
                    // TODO: Implement upvalues
                    let _ = self.read_byte()?;
                    let _ = self.read_byte()?;
                }

                OpCode::GetIter | OpCode::IterNext => {
                    // TODO: Implement iteration
                    return Err(RuntimeError::NotImplemented("iteration".to_string()));
                }
            }
        }
    }

    // === Helper methods ===

    fn binary_op<F>(&mut self, op: F) -> Result<(), RuntimeError>
    where
        F: FnOnce(&Value, &Value) -> Result<Value, RuntimeError>,
    {
        let dst = self.read_byte()?;
        let left = self.read_byte()?;
        let right = self.read_byte()?;

        let left_val = self.get_register(left);
        let right_val = self.get_register(right);
        let result = op(left_val, right_val)?;

        self.set_register(dst, result);
        Ok(())
    }

    fn comparison_op<F>(&mut self, op: F) -> Result<(), RuntimeError>
    where
        F: FnOnce(f64, f64) -> bool,
    {
        let dst = self.read_byte()?;
        let left = self.read_byte()?;
        let right = self.read_byte()?;

        let left_val = self.get_register(left);
        let right_val = self.get_register(right);

        let result = match (left_val, right_val) {
            (Value::Int(a), Value::Int(b)) => op(*a as f64, *b as f64),
            (Value::Float(a), Value::Float(b)) => op(*a, *b),
            (Value::Int(a), Value::Float(b)) => op(*a as f64, *b),
            (Value::Float(a), Value::Int(b)) => op(*a, *b as f64),
            _ => return Err(RuntimeError::TypeError {
                expected: "number".to_string(),
                got: left_val.type_name().to_string(),
            }),
        };

        self.set_register(dst, Value::Bool(result));
        Ok(())
    }

    fn bitwise_op<F>(&mut self, op: F) -> Result<(), RuntimeError>
    where
        F: FnOnce(i64, i64) -> i64,
    {
        let dst = self.read_byte()?;
        let left = self.read_byte()?;
        let right = self.read_byte()?;

        let left_val = self.get_register(left);
        let right_val = self.get_register(right);

        let result = match (left_val, right_val) {
            (Value::Int(a), Value::Int(b)) => Value::Int(op(*a, *b)),
            _ => return Err(RuntimeError::TypeError {
                expected: "int".to_string(),
                got: left_val.type_name().to_string(),
            }),
        };

        self.set_register(dst, result);
        Ok(())
    }

    // === Bytecode reading ===

    fn read_opcode(&mut self) -> Result<OpCode, RuntimeError> {
        let byte = self.read_byte()?;
        OpCode::try_from(byte).map_err(|_| RuntimeError::InvalidOpcode(byte))
    }

    fn read_byte(&mut self) -> Result<u8, RuntimeError> {
        let frame = self.frame();
        let ip = frame.ip;
        let byte = frame.function.chunk.code.get(ip)
            .copied()
            .ok_or(RuntimeError::UnexpectedEnd)?;
        self.frame_mut().ip += 1;
        Ok(byte)
    }

    fn read_u16(&mut self) -> Result<u16, RuntimeError> {
        let lo = self.read_byte()? as u16;
        let hi = self.read_byte()? as u16;
        Ok(lo | (hi << 8))
    }

    fn read_i16(&mut self) -> Result<i16, RuntimeError> {
        Ok(self.read_u16()? as i16)
    }

    fn read_i32(&mut self) -> Result<i32, RuntimeError> {
        let b0 = self.read_byte()? as i32;
        let b1 = self.read_byte()? as i32;
        let b2 = self.read_byte()? as i32;
        let b3 = self.read_byte()? as i32;
        Ok(b0 | (b1 << 8) | (b2 << 16) | (b3 << 24))
    }

    // === Frame/register access ===

    fn frame(&self) -> &CallFrame {
        self.frames.last().unwrap()
    }

    fn frame_mut(&mut self) -> &mut CallFrame {
        self.frames.last_mut().unwrap()
    }

    fn get_register(&self, reg: u8) -> &Value {
        self.frame().get_register(reg)
    }

    fn set_register(&mut self, reg: u8, value: Value) {
        self.frame_mut().set_register(reg, value);
    }

    fn get_constant(&self, idx: u16) -> Result<Value, RuntimeError> {
        self.frame()
            .function
            .chunk
            .constants
            .get(idx as usize)
            .cloned()
            .ok_or(RuntimeError::InvalidConstant(idx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;
    use crate::compiler::Compiler;

    fn eval(source: &str) -> Result<Value, RuntimeError> {
        let lexer = Lexer::new(source);
        let mut parser = Parser::new(lexer);
        let stmts = parser.parse().unwrap();
        let mut compiler = Compiler::new();
        let module = compiler.compile(&stmts).unwrap();
        let mut vm = VM::new();
        vm.run(module)
    }

    #[test]
    fn test_eval_int() {
        assert_eq!(eval("42").unwrap(), Value::Int(42));
    }

    #[test]
    fn test_eval_add() {
        assert_eq!(eval("1 + 2").unwrap(), Value::Int(3));
    }

    #[test]
    fn test_eval_subtract() {
        assert_eq!(eval("10 - 3").unwrap(), Value::Int(7));
    }

    #[test]
    fn test_eval_multiply() {
        assert_eq!(eval("6 * 7").unwrap(), Value::Int(42));
    }

    #[test]
    fn test_eval_divide() {
        assert_eq!(eval("15 / 3").unwrap(), Value::Int(5));
    }

    #[test]
    fn test_eval_precedence() {
        assert_eq!(eval("2 + 3 * 4").unwrap(), Value::Int(14));
    }

    #[test]
    fn test_eval_grouping() {
        assert_eq!(eval("(2 + 3) * 4").unwrap(), Value::Int(20));
    }

    #[test]
    fn test_eval_comparison() {
        assert_eq!(eval("1 < 2").unwrap(), Value::Bool(true));
        assert_eq!(eval("2 > 1").unwrap(), Value::Bool(true));
        assert_eq!(eval("1 == 1").unwrap(), Value::Bool(true));
        assert_eq!(eval("1 != 2").unwrap(), Value::Bool(true));
    }

    #[test]
    fn test_eval_logical() {
        assert_eq!(eval("true and false").unwrap(), Value::Bool(false));
        assert_eq!(eval("true or false").unwrap(), Value::Bool(true));
        assert_eq!(eval("not true").unwrap(), Value::Bool(false));
    }

    #[test]
    fn test_eval_let() {
        assert_eq!(eval("let x = 42\nx").unwrap(), Value::Int(42));
    }

    #[test]
    fn test_eval_assignment() {
        assert_eq!(eval("let mut x = 1\nx = 2\nx").unwrap(), Value::Int(2));
    }

    #[test]
    fn test_eval_if_true() {
        assert_eq!(eval("let x = 0\nif true { x = 1 }\nx").unwrap(), Value::Int(1));
    }

    #[test]
    fn test_eval_if_false() {
        assert_eq!(eval("let x = 0\nif false { x = 1 }\nx").unwrap(), Value::Int(0));
    }

    #[test]
    fn test_eval_while() {
        assert_eq!(
            eval("let mut x = 0\nwhile x < 5 { x = x + 1 }\nx").unwrap(),
            Value::Int(5)
        );
    }

    #[test]
    fn test_eval_string_concat() {
        assert_eq!(
            eval("\"hello\" ++ \" \" ++ \"world\"").unwrap(),
            Value::String("hello world".to_string())
        );
    }

    #[test]
    fn test_eval_float() {
        assert_eq!(eval("3.14").unwrap(), Value::Float(3.14));
    }

    #[test]
    fn test_eval_float_arithmetic() {
        assert_eq!(eval("1.5 + 2.5").unwrap(), Value::Float(4.0));
    }
}
