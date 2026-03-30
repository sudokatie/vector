//! Virtual machine for executing bytecode

pub mod value;
pub mod frame;
pub mod execute;

pub use value::Value;
pub use frame::CallFrame;

use crate::compiler::Function;
use crate::gc::{GC, GCStats};
use crate::jit::{Jit, TypeTag};
use thiserror::Error;
use std::rc::Rc;

#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error("Type error: expected {expected}, got {got}")]
    TypeError { expected: String, got: String },

    #[error("Stack overflow")]
    StackOverflow,

    #[error("Stack underflow")]
    StackUnderflow,

    #[error("Undefined variable '{0}'")]
    UndefinedVariable(String),

    #[error("Division by zero")]
    DivisionByZero,

    #[error("Index out of bounds: {0}")]
    IndexOutOfBounds(i64),

    #[error("Invalid opcode: {0}")]
    InvalidOpcode(u8),

    #[error("Invalid constant index: {0}")]
    InvalidConstant(u16),

    #[error("Unexpected end of bytecode")]
    UnexpectedEnd,

    #[error("Not callable: {0}")]
    NotCallable(String),

    #[error("Arity mismatch: expected {expected}, got {got}")]
    ArityMismatch { expected: u8, got: u8 },

    #[error("Not implemented: {0}")]
    NotImplemented(String),
}

use std::cell::RefCell;
use value::{Closure, Upvalue};

/// The Vector virtual machine
pub struct VM {
    frames: Vec<CallFrame>,
    functions: Vec<Rc<Function>>,
    globals: fnv::FnvHashMap<String, Value>,
    /// JIT compiler (optional)
    jit: Option<Jit>,
    /// Whether JIT is enabled
    jit_enabled: bool,
    /// Garbage collector
    gc: GC,
    /// Current closure's upvalues (for the currently executing closure)
    current_upvalues: Vec<Rc<RefCell<Upvalue>>>,
    /// Open upvalues that haven't been closed yet
    open_upvalues: Vec<Rc<RefCell<Upvalue>>>,
}

impl VM {
    pub fn new() -> Self {
        let mut vm = Self {
            frames: Vec::with_capacity(64),
            functions: Vec::new(),
            globals: fnv::FnvHashMap::default(),
            jit: Some(Jit::new()),
            jit_enabled: true,
            gc: GC::new(),
            current_upvalues: Vec::new(),
            open_upvalues: Vec::new(),
        };
        vm.register_stdlib();
        vm
    }

    /// Create VM without JIT (interpreter only)
    pub fn new_without_jit() -> Self {
        let mut vm = Self {
            frames: Vec::with_capacity(64),
            functions: Vec::new(),
            globals: fnv::FnvHashMap::default(),
            jit: None,
            jit_enabled: false,
            gc: GC::new(),
            current_upvalues: Vec::new(),
            open_upvalues: Vec::new(),
        };
        vm.register_stdlib();
        vm
    }

    /// Create VM with custom heap size
    pub fn with_heap_size(heap_size: usize) -> Self {
        let mut vm = Self {
            frames: Vec::with_capacity(64),
            functions: Vec::new(),
            globals: fnv::FnvHashMap::default(),
            jit: Some(Jit::new()),
            jit_enabled: true,
            gc: GC::with_heap_size(heap_size),
            current_upvalues: Vec::new(),
            open_upvalues: Vec::new(),
        };
        vm.register_stdlib();
        vm
    }

    /// Enable or disable JIT compilation
    pub fn set_jit_enabled(&mut self, enabled: bool) {
        self.jit_enabled = enabled;
        if let Some(jit) = &mut self.jit {
            if enabled {
                jit.enable();
            } else {
                jit.disable();
            }
        }
    }

    /// Get JIT statistics
    pub fn jit_stats(&self) -> Option<&crate::jit::JitStats> {
        self.jit.as_ref().map(|j| &j.stats)
    }

    /// Get profiler statistics
    pub fn profiler_stats(&self) -> Option<&crate::jit::ProfilerStats> {
        self.jit.as_ref().map(|j| &j.profiler.stats)
    }

    /// Get GC statistics
    pub fn gc_stats(&self) -> &GCStats {
        self.gc.stats()
    }

    /// Get heap information: (allocated, max_size, threshold)
    pub fn heap_info(&self) -> (usize, usize, usize) {
        self.gc.heap_info()
    }

    /// Trigger a garbage collection
    pub fn collect_garbage(&mut self) {
        // Enumerate roots from stack and globals
        // Currently using Rc<RefCell> for heap objects, so GC is a no-op
        // This infrastructure is ready for when Value is migrated to use GcRef
        self.gc.collect();
    }

    /// Enable or disable automatic garbage collection
    pub fn set_gc_auto_collect(&mut self, enabled: bool) {
        self.gc.set_auto_collect(enabled);
    }

    fn register_stdlib(&mut self) {
        use crate::runtime::stdlib;
        
        // === Core ===
        self.globals.insert("print".to_string(), Value::NativeFunction(stdlib::print_fn));
        self.globals.insert("type".to_string(), Value::NativeFunction(stdlib::type_fn));
        self.globals.insert("len".to_string(), Value::NativeFunction(stdlib::len_fn));
        self.globals.insert("assert".to_string(), Value::NativeFunction(stdlib::assert_fn));
        self.globals.insert("error".to_string(), Value::NativeFunction(stdlib::error_fn));
        self.globals.insert("str".to_string(), Value::NativeFunction(stdlib::str_fn));
        
        // === String functions ===
        self.globals.insert("upper".to_string(), Value::NativeFunction(stdlib::upper_fn));
        self.globals.insert("lower".to_string(), Value::NativeFunction(stdlib::lower_fn));
        self.globals.insert("trim".to_string(), Value::NativeFunction(stdlib::trim_fn));
        self.globals.insert("split".to_string(), Value::NativeFunction(stdlib::split_fn));
        self.globals.insert("contains".to_string(), Value::NativeFunction(stdlib::contains_fn));
        self.globals.insert("replace".to_string(), Value::NativeFunction(stdlib::replace_fn));
        
        // === Math functions ===
        self.globals.insert("abs".to_string(), Value::NativeFunction(stdlib::abs_fn));
        self.globals.insert("floor".to_string(), Value::NativeFunction(stdlib::floor_fn));
        self.globals.insert("ceil".to_string(), Value::NativeFunction(stdlib::ceil_fn));
        self.globals.insert("sqrt".to_string(), Value::NativeFunction(stdlib::sqrt_fn));
        self.globals.insert("pow".to_string(), Value::NativeFunction(stdlib::pow_fn));
        self.globals.insert("sin".to_string(), Value::NativeFunction(stdlib::sin_fn));
        self.globals.insert("cos".to_string(), Value::NativeFunction(stdlib::cos_fn));
        self.globals.insert("tan".to_string(), Value::NativeFunction(stdlib::tan_fn));
        self.globals.insert("min".to_string(), Value::NativeFunction(stdlib::min_fn));
        self.globals.insert("max".to_string(), Value::NativeFunction(stdlib::max_fn));
        self.globals.insert("random".to_string(), Value::NativeFunction(stdlib::random_fn));
        self.globals.insert("random_int".to_string(), Value::NativeFunction(stdlib::random_int_fn));
        
        // === Array functions ===
        self.globals.insert("push".to_string(), Value::NativeFunction(stdlib::push_fn));
        self.globals.insert("pop".to_string(), Value::NativeFunction(stdlib::pop_fn));
        self.globals.insert("insert".to_string(), Value::NativeFunction(stdlib::insert_fn));
        self.globals.insert("remove".to_string(), Value::NativeFunction(stdlib::remove_fn));
        self.globals.insert("sort".to_string(), Value::NativeFunction(stdlib::sort_fn));
        self.globals.insert("reverse".to_string(), Value::NativeFunction(stdlib::reverse_fn));
        
        // === Higher-order array functions (need VM access) ===
        self.globals.insert("map".to_string(), Value::BuiltinHOF(value::BuiltinHOF::Map));
        self.globals.insert("filter".to_string(), Value::BuiltinHOF(value::BuiltinHOF::Filter));
        self.globals.insert("reduce".to_string(), Value::BuiltinHOF(value::BuiltinHOF::Reduce));
        
        // === Table functions ===
        self.globals.insert("keys".to_string(), Value::NativeFunction(stdlib::keys_fn));
        self.globals.insert("values".to_string(), Value::NativeFunction(stdlib::values_fn));
        self.globals.insert("has_key".to_string(), Value::NativeFunction(stdlib::table_contains_fn));
        self.globals.insert("table_remove".to_string(), Value::NativeFunction(stdlib::table_remove_fn));
        
        // === Namespaced modules ===
        // math module
        let mut math = fnv::FnvHashMap::default();
        math.insert(Value::String("abs".to_string()), Value::NativeFunction(stdlib::abs_fn));
        math.insert(Value::String("floor".to_string()), Value::NativeFunction(stdlib::floor_fn));
        math.insert(Value::String("ceil".to_string()), Value::NativeFunction(stdlib::ceil_fn));
        math.insert(Value::String("sqrt".to_string()), Value::NativeFunction(stdlib::sqrt_fn));
        math.insert(Value::String("pow".to_string()), Value::NativeFunction(stdlib::pow_fn));
        math.insert(Value::String("sin".to_string()), Value::NativeFunction(stdlib::sin_fn));
        math.insert(Value::String("cos".to_string()), Value::NativeFunction(stdlib::cos_fn));
        math.insert(Value::String("tan".to_string()), Value::NativeFunction(stdlib::tan_fn));
        math.insert(Value::String("min".to_string()), Value::NativeFunction(stdlib::min_fn));
        math.insert(Value::String("max".to_string()), Value::NativeFunction(stdlib::max_fn));
        math.insert(Value::String("random".to_string()), Value::NativeFunction(stdlib::random_fn));
        math.insert(Value::String("random_int".to_string()), Value::NativeFunction(stdlib::random_int_fn));
        self.globals.insert("math".to_string(), Value::Table(Rc::new(RefCell::new(math))));
        
        // io module
        let mut io = fnv::FnvHashMap::default();
        io.insert(Value::String("read_file".to_string()), Value::NativeFunction(stdlib::read_file_fn));
        io.insert(Value::String("write_file".to_string()), Value::NativeFunction(stdlib::write_file_fn));
        io.insert(Value::String("read_line".to_string()), Value::NativeFunction(stdlib::read_line_fn));
        io.insert(Value::String("input".to_string()), Value::NativeFunction(stdlib::input_fn));
        self.globals.insert("io".to_string(), Value::Table(Rc::new(RefCell::new(io))));
        
        // === IO functions ===
        self.globals.insert("read_file".to_string(), Value::NativeFunction(stdlib::read_file_fn));
        self.globals.insert("write_file".to_string(), Value::NativeFunction(stdlib::write_file_fn));
        self.globals.insert("read_line".to_string(), Value::NativeFunction(stdlib::read_line_fn));
        self.globals.insert("input".to_string(), Value::NativeFunction(stdlib::input_fn));
    }
}

impl Default for VM {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_creation() {
        let vm = VM::new();
        assert!(vm.globals.contains_key("print"));
        assert!(vm.globals.contains_key("type"));
        assert!(vm.globals.contains_key("len"));
    }

    #[test]
    fn test_vm_with_jit() {
        let vm = VM::new();
        assert!(vm.jit.is_some());
        assert!(vm.jit_enabled);
    }

    #[test]
    fn test_vm_without_jit() {
        let vm = VM::new_without_jit();
        assert!(vm.jit.is_none());
        assert!(!vm.jit_enabled);
    }

    #[test]
    fn test_vm_jit_toggle() {
        let mut vm = VM::new();
        assert!(vm.jit_enabled);

        vm.set_jit_enabled(false);
        assert!(!vm.jit_enabled);

        vm.set_jit_enabled(true);
        assert!(vm.jit_enabled);
    }

    #[test]
    fn test_vm_gc_stats() {
        let vm = VM::new();
        let stats = vm.gc_stats();
        assert_eq!(stats.collections, 0);
    }

    #[test]
    fn test_vm_heap_info() {
        let vm = VM::new();
        let (allocated, max_size, threshold) = vm.heap_info();
        assert_eq!(allocated, 0);
        assert!(max_size > 0);
        assert!(threshold > 0);
    }

    #[test]
    fn test_vm_with_heap_size() {
        let vm = VM::with_heap_size(16 * 1024 * 1024); // 16MB
        let (_, max_size, _) = vm.heap_info();
        assert_eq!(max_size, 16 * 1024 * 1024);
    }

    #[test]
    fn test_vm_collect_garbage() {
        let mut vm = VM::new();
        vm.collect_garbage();
        assert_eq!(vm.gc_stats().collections, 1);
    }

    #[test]
    fn test_vm_gc_auto_collect_toggle() {
        let mut vm = VM::new();
        vm.set_gc_auto_collect(false);
        vm.set_gc_auto_collect(true);
        // Just verify it doesn't panic
    }
}
