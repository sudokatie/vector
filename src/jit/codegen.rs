//! Cranelift code generation

use super::profile::FunctionProfile;
use super::JitError;
use crate::compiler::Function;

use cranelift_codegen::ir::{AbiParam, types, InstBuilder};
use cranelift_codegen::ir::function::Function as CraneliftFunction;
use cranelift_codegen::isa::CallConv;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Module, Linkage};

use std::collections::HashMap;

// Re-export Cranelift Value type with a different name to avoid confusion
type CraneliftValue = cranelift_codegen::ir::Value;

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
}

/// Compile a function to native code
pub fn compile_function(
    func: &Function,
    profile: Option<&FunctionProfile>,
) -> Result<CompiledCode, JitError> {
    // Create JIT module
    let builder = JITBuilder::new(cranelift_module::default_libcall_names())
        .map_err(|e| JitError::Cranelift(e.to_string()))?;

    let mut module = JITModule::new(builder);

    // Create function signature
    // For now: all args are i64, return is i64
    let mut sig = module.make_signature();
    for _ in 0..func.arity {
        sig.params.push(AbiParam::new(types::I64));
    }
    sig.returns.push(AbiParam::new(types::I64));

    // Declare function
    let func_id = module
        .declare_function(&func.name, Linkage::Local, &sig)
        .map_err(|e| JitError::Cranelift(e.to_string()))?;

    // Create function builder context
    let mut ctx = module.make_context();
    ctx.func.signature = sig;

    // Build the function
    {
        let mut builder_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);

        // Create entry block
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        // Get parameters
        let params: Vec<CraneliftValue> = builder.block_params(entry_block).to_vec();

        // For now, just return a constant or first param
        // TODO: Full bytecode translation
        let result = if params.is_empty() {
            builder.ins().iconst(types::I64, 0)
        } else {
            params[0]
        };

        builder.ins().return_(&[result]);
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
        func_id: 0, // Will be set by caller
        entry: code,
        size: 64, // Approximate
        _module: Some(module),
    })
}

/// Bytecode translator for generating Cranelift IR
pub struct BytecodeTranslator<'a> {
    func: &'a Function,
    profile: Option<&'a FunctionProfile>,
    builder: Option<FunctionBuilder<'a>>,

    /// Mapping from register to Cranelift value
    registers: HashMap<u8, CraneliftValue>,

    /// Current bytecode offset
    offset: usize,
}

impl<'a> BytecodeTranslator<'a> {
    pub fn new(func: &'a Function, profile: Option<&'a FunctionProfile>) -> Self {
        Self {
            func,
            profile,
            builder: None,
            registers: HashMap::new(),
            offset: 0,
        }
    }

    /// Read next byte from bytecode
    fn read_byte(&mut self) -> Option<u8> {
        let byte = self.func.chunk.code.get(self.offset).copied();
        self.offset += 1;
        byte
    }

    /// Read u16 from bytecode
    fn read_u16(&mut self) -> Option<u16> {
        let lo = self.read_byte()? as u16;
        let hi = self.read_byte()? as u16;
        Some(lo | (hi << 8))
    }

    /// Read i32 from bytecode
    fn read_i32(&mut self) -> Option<i32> {
        let b0 = self.read_byte()? as i32;
        let b1 = self.read_byte()? as i32;
        let b2 = self.read_byte()? as i32;
        let b3 = self.read_byte()? as i32;
        Some(b0 | (b1 << 8) | (b2 << 16) | (b3 << 24))
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
        assert!(!compiled.entry.is_null());
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
        assert!(stub.entry.is_null());
        assert_eq!(stub.code_size(), 0);
    }
}
