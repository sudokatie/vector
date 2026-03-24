//! JIT compilation via Cranelift

pub mod profile;
pub mod codegen;
pub mod cache;

pub use profile::{Profiler, ProfilerStats, TypeTag, DominantType};
pub use cache::CodeCache;

use crate::compiler::Function;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum JitError {
    #[error("Compilation failed: {0}")]
    CompilationFailed(String),

    #[error("Unsupported opcode: {0}")]
    UnsupportedOpcode(u8),

    #[error("Function not found: {0}")]
    FunctionNotFound(usize),

    #[error("Cranelift error: {0}")]
    Cranelift(String),
}

/// JIT compilation coordinator
pub struct Jit {
    /// Whether JIT is enabled
    enabled: bool,

    /// Hot path profiler
    pub profiler: Profiler,

    /// Compiled code cache
    pub cache: CodeCache,

    /// JIT compilation statistics
    pub stats: JitStats,
}

#[derive(Debug, Clone, Default)]
pub struct JitStats {
    pub functions_compiled: usize,
    pub compilation_time_us: u64,
    pub native_calls: u64,
    pub interpreter_fallbacks: u64,
}

impl Jit {
    pub fn new() -> Self {
        Self {
            enabled: true,
            profiler: Profiler::new(),
            cache: CodeCache::new(),
            stats: JitStats::default(),
        }
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Check if a function should be JIT compiled
    pub fn should_compile(&self, func_id: usize) -> bool {
        self.enabled
            && !self.cache.contains(func_id)
            && self.profiler.is_hot(func_id)
    }

    /// Try to compile a function
    pub fn try_compile(&mut self, func_id: usize, func: &Function) -> Result<(), JitError> {
        if !self.enabled {
            return Ok(());
        }

        if self.cache.contains(func_id) {
            return Ok(()); // Already compiled
        }

        let profile = self.profiler.get_profile(func_id);

        let start = std::time::Instant::now();
        let result = codegen::compile_function(func, profile);
        let elapsed = start.elapsed().as_micros() as u64;

        match result {
            Ok(compiled) => {
                self.cache.insert(func_id, compiled);
                self.profiler.mark_compiled(func_id);
                self.stats.functions_compiled += 1;
                self.stats.compilation_time_us += elapsed;
                Ok(())
            }
            Err(e) => {
                self.profiler.mark_compile_failed(func_id);
                Err(e)
            }
        }
    }

    /// Get compiled code for a function, if available
    pub fn get_compiled(&self, func_id: usize) -> Option<&codegen::CompiledCode> {
        self.cache.get(func_id)
    }

    /// Record a function call for profiling
    pub fn record_call(&mut self, func_id: usize) {
        if self.enabled {
            self.profiler.record_call(func_id);
        }
    }

    /// Record a loop iteration for profiling
    pub fn record_loop(&mut self, func_id: usize, loop_offset: usize) {
        if self.enabled {
            self.profiler.record_loop(func_id, loop_offset);
        }
    }

    /// Initialize profiling for a function
    pub fn init_function(&mut self, func_id: usize, arity: u8) {
        self.profiler.init_function(func_id, arity);
    }

    /// Process the compile queue, compiling hot functions
    pub fn process_queue(&mut self, functions: &[std::rc::Rc<Function>]) {
        while let Some(func_id) = self.profiler.next_compile_candidate() {
            if let Some(func) = functions.get(func_id) {
                if let Err(e) = self.try_compile(func_id, func) {
                    // Log error but continue
                    eprintln!("JIT compilation failed for function {}: {}", func_id, e);
                }
            }
        }
    }
}

impl Default for Jit {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jit_disabled() {
        let mut jit = Jit::new();
        jit.disable();
        assert!(!jit.is_enabled());
        assert!(!jit.should_compile(0));
    }

    #[test]
    fn test_jit_should_compile() {
        let mut jit = Jit::new();
        jit.profiler.hot_threshold = 5;
        jit.init_function(0, 0);

        // Not hot yet
        assert!(!jit.should_compile(0));

        // Make hot
        for _ in 0..5 {
            jit.record_call(0);
        }

        assert!(jit.should_compile(0));
    }
}
