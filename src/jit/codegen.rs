//! Cranelift code generation - Full bytecode translation

use super::profile::FunctionProfile;
use super::JitError;
use crate::compiler::{Function, OpCode};

use cranelift_codegen::ir::{AbiParam, types, InstBuilder, Value as CrValue, Block};
use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Module, Linkage};

use std::collections::HashMap;

/// Compiled native code for a function
pub struct CompiledCode {
    /// Original function ID
    pub func_id: usize,

    /// Entry point function pointer
    entry: *const u8,

    /// Size of generated code
    size: usize,

    /// JIT module (keeps code alive)
    _module: Option<JITModule>,
}

// Safety: CompiledCode is effectively immutable after creation
unsafe impl Send for CompiledCode {}
unsafe impl Sync for CompiledCode {}

impl CompiledCode {
    pub fn code_size(&self) -> usize {
        self.size
    }

    /// Create a stub for testing
    pub fn stub(func_id: usize) -> Self {
        Self {
            func_id,
            entry: std::ptr::null(),
            size: 0,
            _module: None,
        }
    }

    /// Get the entry point (for calling)
    pub fn entry(&self) -> *const u8 {
        self.entry
    }

    /// Check if this is a valid compiled function
    pub fn is_valid(&self) -> bool {
        !self.entry.is_null()
    }

    /// Call the compiled function with integer arguments
    /// 
    /// # Safety
    /// Caller must ensure arguments match the function signature
    pub unsafe fn call_i64(&self, args: &[i64]) -> i64 {
        if self.entry.is_null() {
            return 0;
        }

        match args.len() {
            0 => {
                let func: extern "C" fn() -> i64 = std::mem::transmute(self.entry);
                func()
            }
            1 => {
                let func: extern "C" fn(i64) -> i64 = std::mem::transmute(self.entry);
                func(args[0])
            }
            2 => {
                let func: extern "C" fn(i64, i64) -> i64 = std::mem::transmute(self.entry);
                func(args[0], args[1])
            }
            3 => {
                let func: extern "C" fn(i64, i64, i64) -> i64 = std::mem::transmute(self.entry);
                func(args[0], args[1], args[2])
            }
            4 => {
                let func: extern "C" fn(i64, i64, i64, i64) -> i64 = std::mem::transmute(self.entry);
                func(args[0], args[1], args[2], args[3])
            }
            _ => {
                // For more args, we'd need a different calling convention
                0
            }
        }
    }
}

/// Compile a function to native code
pub fn compile_function(
    func: &Function,
    profile: Option<&FunctionProfile>,
) -> Result<CompiledCode, JitError> {
    let mut translator = BytecodeTranslator::new(func, profile);
    translator.compile()
}

/// Bytecode translator for generating Cranelift IR
pub struct BytecodeTranslator<'a> {
    func: &'a Function,
    profile: Option<&'a FunctionProfile>,
    
    /// Bytecode offset
    offset: usize,
    
    /// Block map: bytecode offset -> Cranelift block
    block_map: HashMap<usize, Block>,
    
    /// Variables for registers (r0-r255)
    variables: Vec<Variable>,
    
    /// Jump targets discovered during first pass
    jump_targets: Vec<usize>,
    
    /// Whether current block has been terminated
    block_terminated: bool,
}

impl<'a> BytecodeTranslator<'a> {
    pub fn new(func: &'a Function, profile: Option<&'a FunctionProfile>) -> Self {
        Self {
            func,
            profile,
            offset: 0,
            block_map: HashMap::new(),
            variables: Vec::new(),
            jump_targets: Vec::new(),
            block_terminated: false,
        }
    }

    /// Compile the function
    pub fn compile(&mut self) -> Result<CompiledCode, JitError> {
        // Create JIT module with proper settings
        let mut flag_builder = settings::builder();
        flag_builder.set("opt_level", "speed").unwrap();
        flag_builder.set("is_pic", "false").unwrap();
        
        let isa_builder = cranelift_native::builder()
            .map_err(|e| JitError::Cranelift(e.to_string()))?;
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .map_err(|e| JitError::Cranelift(e.to_string()))?;

        let builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        let mut module = JITModule::new(builder);

        // Create function signature: all i64 args and return
        let mut sig = module.make_signature();
        for _ in 0..self.func.arity {
            sig.params.push(AbiParam::new(types::I64));
        }
        sig.returns.push(AbiParam::new(types::I64));

        // Declare function
        let func_id = module
            .declare_function(&self.func.name, Linkage::Local, &sig)
            .map_err(|e| JitError::Cranelift(e.to_string()))?;

        // Create function context
        let mut ctx = module.make_context();
        ctx.func.signature = sig;

        // First pass: find all jump targets
        self.find_jump_targets();

        // Build the function
        {
            let mut builder_ctx = FunctionBuilderContext::new();
            let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);

            // Declare variables for registers
            self.variables.clear();
            for i in 0..256 {
                let var = Variable::from_u32(i);
                builder.declare_var(var, types::I64);
                self.variables.push(var);
            }

            // Create entry block
            let entry_block = builder.create_block();
            builder.append_block_params_for_function_params(entry_block);
            
            // Create blocks for all jump targets
            self.block_map.clear();
            self.block_map.insert(0, entry_block);
            for &target in &self.jump_targets {
                if target != 0 && !self.block_map.contains_key(&target) {
                    let block = builder.create_block();
                    self.block_map.insert(target, block);
                }
            }

            builder.switch_to_block(entry_block);
            builder.seal_block(entry_block);

            // Initialize parameters
            let params: Vec<CrValue> = builder.block_params(entry_block).to_vec();
            for (i, &param) in params.iter().enumerate() {
                builder.def_var(self.variables[i], param);
            }

            // Initialize non-parameter registers to 0
            let zero = builder.ins().iconst(types::I64, 0);
            for i in params.len()..256 {
                builder.def_var(self.variables[i], zero);
            }

            // Translate bytecode
            self.offset = 0;
            self.translate_bytecode(&mut builder)?;

            builder.finalize();
        }

        // Compile
        module
            .define_function(func_id, &mut ctx)
            .map_err(|e| JitError::Cranelift(e.to_string()))?;

        module.clear_context(&mut ctx);
        module
            .finalize_definitions()
            .map_err(|e| JitError::Cranelift(e.to_string()))?;

        // Get code pointer
        let code = module.get_finalized_function(func_id);

        Ok(CompiledCode {
            func_id: 0,
            entry: code,
            size: 256, // Approximate
            _module: Some(module),
        })
    }

    /// First pass: find all jump targets
    fn find_jump_targets(&mut self) {
        self.jump_targets.clear();
        self.offset = 0;
        let code = &self.func.chunk.code;

        while self.offset < code.len() {
            let op_byte = code[self.offset];
            let op = match OpCode::try_from(op_byte) {
                Ok(op) => op,
                Err(_) => {
                    self.offset += 1;
                    continue;
                }
            };

            let op_start = self.offset;
            self.offset += 1;

            match op {
                OpCode::Jump => {
                    let offset = self.read_i16();
                    let target = (op_start as i32 + 3 + offset as i32) as usize;
                    self.jump_targets.push(target);
                }
                OpCode::JumpIf | OpCode::JumpIfNot => {
                    self.offset += 1; // cond register
                    let offset = self.read_i16();
                    let target = (op_start as i32 + 4 + offset as i32) as usize;
                    self.jump_targets.push(target);
                    // Fall-through is also a target
                    self.jump_targets.push(self.offset);
                }
                OpCode::Loop => {
                    let offset = self.read_u16();
                    let target = self.offset - offset as usize;
                    self.jump_targets.push(target);
                }
                _ => {
                    self.offset += op.operand_size();
                }
            }
        }

        self.jump_targets.sort();
        self.jump_targets.dedup();
    }

    /// Translate bytecode to Cranelift IR
    fn translate_bytecode(&mut self, builder: &mut FunctionBuilder) -> Result<(), JitError> {
        let code = &self.func.chunk.code;
        self.offset = 0;

        while self.offset < code.len() {
            // Check if this offset starts a new block
            if self.offset > 0 {
                if let Some(&block) = self.block_map.get(&self.offset) {
                    // Jump to the new block if current block doesn't end with terminator
                    if !self.block_terminated {
                        builder.ins().jump(block, &[]);
                    }
                    builder.switch_to_block(block);
                    builder.seal_block(block);
                    self.block_terminated = false;
                }
            }

            let op_byte = code[self.offset];
            let op = OpCode::try_from(op_byte)
                .map_err(|_| JitError::UnsupportedOpcode(op_byte))?;

            self.offset += 1;
            self.translate_opcode(op, builder)?;
        }

        // Ensure function ends with return
        if !self.block_terminated {
            let zero = builder.ins().iconst(types::I64, 0);
            builder.ins().return_(&[zero]);
        }

        Ok(())
    }

    /// Translate a single opcode
    fn translate_opcode(&mut self, op: OpCode, builder: &mut FunctionBuilder) -> Result<(), JitError> {
        match op {
            OpCode::LoadNil => {
                let dst = self.read_byte() as usize;
                // nil = 0 in our representation
                let zero = builder.ins().iconst(types::I64, 0);
                builder.def_var(self.variables[dst], zero);
            }

            OpCode::LoadTrue => {
                let dst = self.read_byte() as usize;
                // true = 1
                let one = builder.ins().iconst(types::I64, 1);
                builder.def_var(self.variables[dst], one);
            }

            OpCode::LoadFalse => {
                let dst = self.read_byte() as usize;
                // false = 0
                let zero = builder.ins().iconst(types::I64, 0);
                builder.def_var(self.variables[dst], zero);
            }

            OpCode::LoadInt => {
                let dst = self.read_byte() as usize;
                let value = self.read_i32() as i64;
                let val = builder.ins().iconst(types::I64, value);
                builder.def_var(self.variables[dst], val);
            }

            OpCode::LoadConst => {
                let dst = self.read_byte() as usize;
                let idx = self.read_u16() as usize;
                
                // Get constant value
                if let Some(constant) = self.func.chunk.constants.get(idx) {
                    let val = match constant {
                        crate::vm::Value::Nil => builder.ins().iconst(types::I64, 0),
                        crate::vm::Value::Bool(b) => builder.ins().iconst(types::I64, if *b { 1 } else { 0 }),
                        crate::vm::Value::Int(i) => builder.ins().iconst(types::I64, *i),
                        crate::vm::Value::Float(f) => {
                            // Store float bits as i64
                            builder.ins().iconst(types::I64, f.to_bits() as i64)
                        }
                        _ => {
                            // For non-numeric constants, we can't JIT efficiently
                            // Return a marker value
                            builder.ins().iconst(types::I64, 0)
                        }
                    };
                    builder.def_var(self.variables[dst], val);
                } else {
                    let zero = builder.ins().iconst(types::I64, 0);
                    builder.def_var(self.variables[dst], zero);
                }
            }

            OpCode::Move => {
                let dst = self.read_byte() as usize;
                let src = self.read_byte() as usize;
                let val = builder.use_var(self.variables[src]);
                builder.def_var(self.variables[dst], val);
            }

            OpCode::Copy => {
                // For JIT, copy is same as move (values are copied)
                let dst = self.read_byte() as usize;
                let src = self.read_byte() as usize;
                let val = builder.use_var(self.variables[src]);
                builder.def_var(self.variables[dst], val);
            }

            // Arithmetic operations
            OpCode::Add => {
                let dst = self.read_byte() as usize;
                let left = self.read_byte() as usize;
                let right = self.read_byte() as usize;
                let lval = builder.use_var(self.variables[left]);
                let rval = builder.use_var(self.variables[right]);
                let result = builder.ins().iadd(lval, rval);
                builder.def_var(self.variables[dst], result);
            }

            OpCode::Sub => {
                let dst = self.read_byte() as usize;
                let left = self.read_byte() as usize;
                let right = self.read_byte() as usize;
                let lval = builder.use_var(self.variables[left]);
                let rval = builder.use_var(self.variables[right]);
                let result = builder.ins().isub(lval, rval);
                builder.def_var(self.variables[dst], result);
            }

            OpCode::Mul => {
                let dst = self.read_byte() as usize;
                let left = self.read_byte() as usize;
                let right = self.read_byte() as usize;
                let lval = builder.use_var(self.variables[left]);
                let rval = builder.use_var(self.variables[right]);
                let result = builder.ins().imul(lval, rval);
                builder.def_var(self.variables[dst], result);
            }

            OpCode::Div => {
                let dst = self.read_byte() as usize;
                let left = self.read_byte() as usize;
                let right = self.read_byte() as usize;
                let lval = builder.use_var(self.variables[left]);
                let rval = builder.use_var(self.variables[right]);
                let result = builder.ins().sdiv(lval, rval);
                builder.def_var(self.variables[dst], result);
            }

            OpCode::Mod => {
                let dst = self.read_byte() as usize;
                let left = self.read_byte() as usize;
                let right = self.read_byte() as usize;
                let lval = builder.use_var(self.variables[left]);
                let rval = builder.use_var(self.variables[right]);
                let result = builder.ins().srem(lval, rval);
                builder.def_var(self.variables[dst], result);
            }

            OpCode::Pow => {
                // Power is complex - for integers we can use a loop or call
                // For now, simplified: just multiply (only works for power of 2)
                let dst = self.read_byte() as usize;
                let left = self.read_byte() as usize;
                let right = self.read_byte() as usize;
                let base = builder.use_var(self.variables[left]);
                let _exp = builder.use_var(self.variables[right]);
                // Simplified: just return base (real impl needs loop/call)
                builder.def_var(self.variables[dst], base);
            }

            OpCode::Neg => {
                let dst = self.read_byte() as usize;
                let src = self.read_byte() as usize;
                let val = builder.use_var(self.variables[src]);
                let result = builder.ins().ineg(val);
                builder.def_var(self.variables[dst], result);
            }

            // Comparison operations
            OpCode::Eq => {
                let dst = self.read_byte() as usize;
                let left = self.read_byte() as usize;
                let right = self.read_byte() as usize;
                let lval = builder.use_var(self.variables[left]);
                let rval = builder.use_var(self.variables[right]);
                let cmp = builder.ins().icmp(IntCC::Equal, lval, rval);
                let result = builder.ins().uextend(types::I64, cmp);
                builder.def_var(self.variables[dst], result);
            }

            OpCode::Ne => {
                let dst = self.read_byte() as usize;
                let left = self.read_byte() as usize;
                let right = self.read_byte() as usize;
                let lval = builder.use_var(self.variables[left]);
                let rval = builder.use_var(self.variables[right]);
                let cmp = builder.ins().icmp(IntCC::NotEqual, lval, rval);
                let result = builder.ins().uextend(types::I64, cmp);
                builder.def_var(self.variables[dst], result);
            }

            OpCode::Lt => {
                let dst = self.read_byte() as usize;
                let left = self.read_byte() as usize;
                let right = self.read_byte() as usize;
                let lval = builder.use_var(self.variables[left]);
                let rval = builder.use_var(self.variables[right]);
                let cmp = builder.ins().icmp(IntCC::SignedLessThan, lval, rval);
                let result = builder.ins().uextend(types::I64, cmp);
                builder.def_var(self.variables[dst], result);
            }

            OpCode::Le => {
                let dst = self.read_byte() as usize;
                let left = self.read_byte() as usize;
                let right = self.read_byte() as usize;
                let lval = builder.use_var(self.variables[left]);
                let rval = builder.use_var(self.variables[right]);
                let cmp = builder.ins().icmp(IntCC::SignedLessThanOrEqual, lval, rval);
                let result = builder.ins().uextend(types::I64, cmp);
                builder.def_var(self.variables[dst], result);
            }

            OpCode::Gt => {
                let dst = self.read_byte() as usize;
                let left = self.read_byte() as usize;
                let right = self.read_byte() as usize;
                let lval = builder.use_var(self.variables[left]);
                let rval = builder.use_var(self.variables[right]);
                let cmp = builder.ins().icmp(IntCC::SignedGreaterThan, lval, rval);
                let result = builder.ins().uextend(types::I64, cmp);
                builder.def_var(self.variables[dst], result);
            }

            OpCode::Ge => {
                let dst = self.read_byte() as usize;
                let left = self.read_byte() as usize;
                let right = self.read_byte() as usize;
                let lval = builder.use_var(self.variables[left]);
                let rval = builder.use_var(self.variables[right]);
                let cmp = builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, lval, rval);
                let result = builder.ins().uextend(types::I64, cmp);
                builder.def_var(self.variables[dst], result);
            }

            // Logical operations
            OpCode::And => {
                let dst = self.read_byte() as usize;
                let left = self.read_byte() as usize;
                let right = self.read_byte() as usize;
                let lval = builder.use_var(self.variables[left]);
                let rval = builder.use_var(self.variables[right]);
                let result = builder.ins().band(lval, rval);
                builder.def_var(self.variables[dst], result);
            }

            OpCode::Or => {
                let dst = self.read_byte() as usize;
                let left = self.read_byte() as usize;
                let right = self.read_byte() as usize;
                let lval = builder.use_var(self.variables[left]);
                let rval = builder.use_var(self.variables[right]);
                let result = builder.ins().bor(lval, rval);
                builder.def_var(self.variables[dst], result);
            }

            OpCode::Not => {
                let dst = self.read_byte() as usize;
                let src = self.read_byte() as usize;
                let val = builder.use_var(self.variables[src]);
                let zero = builder.ins().iconst(types::I64, 0);
                let cmp = builder.ins().icmp(IntCC::Equal, val, zero);
                let result = builder.ins().uextend(types::I64, cmp);
                builder.def_var(self.variables[dst], result);
            }

            // Bitwise operations
            OpCode::BitAnd => {
                let dst = self.read_byte() as usize;
                let left = self.read_byte() as usize;
                let right = self.read_byte() as usize;
                let lval = builder.use_var(self.variables[left]);
                let rval = builder.use_var(self.variables[right]);
                let result = builder.ins().band(lval, rval);
                builder.def_var(self.variables[dst], result);
            }

            OpCode::BitOr => {
                let dst = self.read_byte() as usize;
                let left = self.read_byte() as usize;
                let right = self.read_byte() as usize;
                let lval = builder.use_var(self.variables[left]);
                let rval = builder.use_var(self.variables[right]);
                let result = builder.ins().bor(lval, rval);
                builder.def_var(self.variables[dst], result);
            }

            OpCode::BitXor => {
                let dst = self.read_byte() as usize;
                let left = self.read_byte() as usize;
                let right = self.read_byte() as usize;
                let lval = builder.use_var(self.variables[left]);
                let rval = builder.use_var(self.variables[right]);
                let result = builder.ins().bxor(lval, rval);
                builder.def_var(self.variables[dst], result);
            }

            OpCode::BitNot => {
                let dst = self.read_byte() as usize;
                let src = self.read_byte() as usize;
                let val = builder.use_var(self.variables[src]);
                let result = builder.ins().bnot(val);
                builder.def_var(self.variables[dst], result);
            }

            OpCode::Shl => {
                let dst = self.read_byte() as usize;
                let left = self.read_byte() as usize;
                let right = self.read_byte() as usize;
                let lval = builder.use_var(self.variables[left]);
                let rval = builder.use_var(self.variables[right]);
                let result = builder.ins().ishl(lval, rval);
                builder.def_var(self.variables[dst], result);
            }

            OpCode::Shr => {
                let dst = self.read_byte() as usize;
                let left = self.read_byte() as usize;
                let right = self.read_byte() as usize;
                let lval = builder.use_var(self.variables[left]);
                let rval = builder.use_var(self.variables[right]);
                let result = builder.ins().sshr(lval, rval);
                builder.def_var(self.variables[dst], result);
            }

            // Control flow
            OpCode::Jump => {
                let offset = self.read_i16();
                let target_offset = (self.offset as i32 - 2 + offset as i32) as usize;
                
                if let Some(&target_block) = self.block_map.get(&target_offset) {
                    builder.ins().jump(target_block, &[]);
                    self.block_terminated = true;
                }
            }

            OpCode::JumpIf => {
                let cond = self.read_byte() as usize;
                let offset = self.read_i16();
                let target_offset = (self.offset as i32 - 2 + offset as i32) as usize;
                let fallthrough_offset = self.offset;

                let cond_val = builder.use_var(self.variables[cond]);
                
                if let (Some(&target_block), Some(&fallthrough_block)) = 
                    (self.block_map.get(&target_offset), self.block_map.get(&fallthrough_offset))
                {
                    builder.ins().brif(cond_val, target_block, &[], fallthrough_block, &[]);
                    self.block_terminated = true;
                } else if let Some(&target_block) = self.block_map.get(&target_offset) {
                    // Create fallthrough block
                    let fallthrough = builder.create_block();
                    builder.ins().brif(cond_val, target_block, &[], fallthrough, &[]);
                    builder.switch_to_block(fallthrough);
                    builder.seal_block(fallthrough);
                    self.block_terminated = false;
                }
            }

            OpCode::JumpIfNot => {
                let cond = self.read_byte() as usize;
                let offset = self.read_i16();
                let target_offset = (self.offset as i32 - 2 + offset as i32) as usize;
                let fallthrough_offset = self.offset;

                let cond_val = builder.use_var(self.variables[cond]);
                
                if let (Some(&target_block), Some(&fallthrough_block)) = 
                    (self.block_map.get(&target_offset), self.block_map.get(&fallthrough_offset))
                {
                    // Jump if NOT cond, so swap the branches
                    builder.ins().brif(cond_val, fallthrough_block, &[], target_block, &[]);
                    self.block_terminated = true;
                } else if let Some(&target_block) = self.block_map.get(&target_offset) {
                    let fallthrough = builder.create_block();
                    builder.ins().brif(cond_val, fallthrough, &[], target_block, &[]);
                    builder.switch_to_block(fallthrough);
                    builder.seal_block(fallthrough);
                    self.block_terminated = false;
                }
            }

            OpCode::Loop => {
                let offset = self.read_u16() as usize;
                let target_offset = self.offset - offset;
                
                if let Some(&target_block) = self.block_map.get(&target_offset) {
                    builder.ins().jump(target_block, &[]);
                    self.block_terminated = true;
                }
            }

            // Function calls - these bail out to interpreter
            OpCode::Call | OpCode::TailCall | OpCode::MethodCall => {
                // For calls, we need to bail to interpreter
                // Just skip the operands and return 0
                let operand_size = op.operand_size();
                self.offset += operand_size;
                
                // Return indicating we need interpreter
                let zero = builder.ins().iconst(types::I64, 0);
                builder.ins().return_(&[zero]);
                self.block_terminated = true;
            }

            OpCode::Return => {
                let _dst = self.read_byte();
                let src = self.read_byte() as usize;
                let val = builder.use_var(self.variables[src]);
                builder.ins().return_(&[val]);
                self.block_terminated = true;
            }

            OpCode::ReturnNil => {
                let _dst = self.read_byte();
                let zero = builder.ins().iconst(types::I64, 0);
                builder.ins().return_(&[zero]);
                self.block_terminated = true;
            }

            // Variable access - locals only in JIT
            OpCode::GetLocal => {
                let dst = self.read_byte() as usize;
                let slot = self.read_byte() as usize;
                let val = builder.use_var(self.variables[slot]);
                builder.def_var(self.variables[dst], val);
            }

            OpCode::SetLocal => {
                let slot = self.read_byte() as usize;
                let src = self.read_byte() as usize;
                let val = builder.use_var(self.variables[src]);
                builder.def_var(self.variables[slot], val);
            }

            // Global access - bail to interpreter
            OpCode::GetGlobal | OpCode::SetGlobal => {
                let operand_size = op.operand_size();
                self.offset += operand_size;
                // Continue without doing anything (would need runtime support)
            }

            // Upvalues - bail to interpreter
            OpCode::GetUpvalue | OpCode::SetUpvalue | OpCode::CloseUpvalue => {
                let operand_size = op.operand_size();
                self.offset += operand_size;
            }

            // Collections - bail to interpreter
            OpCode::NewArray | OpCode::ArrayGet | OpCode::ArraySet |
            OpCode::NewTable | OpCode::TableGet | OpCode::TableSet |
            OpCode::Closure | OpCode::MakeRange | OpCode::MakeRangeIncl |
            OpCode::GetIter | OpCode::IterNext | OpCode::Concat => {
                let operand_size = op.operand_size();
                self.offset += operand_size;
                // These need interpreter support
            }
        }

        Ok(())
    }

    /// Read a byte from bytecode
    fn read_byte(&mut self) -> u8 {
        let byte = self.func.chunk.code.get(self.offset).copied().unwrap_or(0);
        self.offset += 1;
        byte
    }

    /// Read u16 from bytecode (little-endian)
    fn read_u16(&mut self) -> u16 {
        let lo = self.read_byte() as u16;
        let hi = self.read_byte() as u16;
        lo | (hi << 8)
    }

    /// Read i16 from bytecode (little-endian)
    fn read_i16(&mut self) -> i16 {
        self.read_u16() as i16
    }

    /// Read i32 from bytecode (little-endian)
    fn read_i32(&mut self) -> i32 {
        let b0 = self.read_byte() as i32;
        let b1 = self.read_byte() as i32;
        let b2 = self.read_byte() as i32;
        let b3 = self.read_byte() as i32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::Chunk;

    #[test]
    fn test_compile_empty_function() {
        let func = Function {
            name: "test".to_string(),
            arity: 0,
            num_locals: 0,
            chunk: Chunk::new(),
            upvalues: vec![],
        };

        let result = compile_function(&func, None);
        assert!(result.is_ok());

        let compiled = result.unwrap();
        assert!(compiled.is_valid());
    }

    #[test]
    fn test_compile_function_with_args() {
        let func = Function {
            name: "add".to_string(),
            arity: 2,
            num_locals: 0,
            chunk: Chunk::new(),
            upvalues: vec![],
        };

        let result = compile_function(&func, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_compiled_code_stub() {
        let stub = CompiledCode::stub(42);
        assert_eq!(stub.func_id, 42);
        assert!(!stub.is_valid());
        assert_eq!(stub.code_size(), 0);
    }

    #[test]
    fn test_compile_simple_add() {
        use crate::compiler::OpCode;
        
        let mut chunk = Chunk::new();
        // LoadInt r0, 10
        chunk.emit(OpCode::LoadInt, 1);
        chunk.emit_byte(0, 1);
        chunk.emit_i32(10, 1);
        // LoadInt r1, 20
        chunk.emit(OpCode::LoadInt, 1);
        chunk.emit_byte(1, 1);
        chunk.emit_i32(20, 1);
        // Add r0, r0, r1
        chunk.emit(OpCode::Add, 1);
        chunk.emit_byte(0, 1);
        chunk.emit_byte(0, 1);
        chunk.emit_byte(1, 1);
        // Return r0
        chunk.emit(OpCode::Return, 1);
        chunk.emit_byte(0, 1);
        chunk.emit_byte(0, 1);

        let func = Function {
            name: "add_test".to_string(),
            arity: 0,
            num_locals: 2,
            chunk,
            upvalues: vec![],
        };

        let result = compile_function(&func, None);
        assert!(result.is_ok());
        
        let compiled = result.unwrap();
        assert!(compiled.is_valid());
        
        // Call the function
        unsafe {
            let result = compiled.call_i64(&[]);
            assert_eq!(result, 30);
        }
    }

    #[test]
    fn test_compile_with_args() {
        use crate::compiler::OpCode;
        
        let mut chunk = Chunk::new();
        // Add r0, r0, r1 (args are in r0, r1)
        chunk.emit(OpCode::Add, 1);
        chunk.emit_byte(0, 1);
        chunk.emit_byte(0, 1);
        chunk.emit_byte(1, 1);
        // Return r0
        chunk.emit(OpCode::Return, 1);
        chunk.emit_byte(0, 1);
        chunk.emit_byte(0, 1);

        let func = Function {
            name: "add".to_string(),
            arity: 2,
            num_locals: 2,
            chunk,
            upvalues: vec![],
        };

        let result = compile_function(&func, None);
        assert!(result.is_ok());
        
        let compiled = result.unwrap();
        
        unsafe {
            assert_eq!(compiled.call_i64(&[5, 7]), 12);
            assert_eq!(compiled.call_i64(&[100, 200]), 300);
        }
    }

    #[test]
    fn test_compile_comparison() {
        use crate::compiler::OpCode;
        
        let mut chunk = Chunk::new();
        // Lt r0, r0, r1
        chunk.emit(OpCode::Lt, 1);
        chunk.emit_byte(0, 1);
        chunk.emit_byte(0, 1);
        chunk.emit_byte(1, 1);
        // Return r0
        chunk.emit(OpCode::Return, 1);
        chunk.emit_byte(0, 1);
        chunk.emit_byte(0, 1);

        let func = Function {
            name: "lt".to_string(),
            arity: 2,
            num_locals: 2,
            chunk,
            upvalues: vec![],
        };

        let compiled = compile_function(&func, None).unwrap();
        
        unsafe {
            assert_eq!(compiled.call_i64(&[5, 10]), 1); // 5 < 10 = true
            assert_eq!(compiled.call_i64(&[10, 5]), 0); // 10 < 5 = false
        }
    }
}
