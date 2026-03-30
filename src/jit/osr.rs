//! On-Stack Replacement (OSR) and Deoptimization support

use std::collections::HashMap;
use fnv::FnvHashMap;

/// OSR entry point - where we can enter JIT code from interpreter
#[derive(Debug, Clone)]
pub struct OSREntryPoint {
    /// Bytecode offset where OSR can occur
    pub bytecode_offset: usize,
    /// Native code offset to jump to
    pub native_offset: usize,
    /// Required register state mapping: bytecode register -> expected value slot
    pub register_map: Vec<RegisterMapping>,
    /// Loop depth at this point
    pub loop_depth: u32,
}

/// Mapping from bytecode register to native location
#[derive(Debug, Clone, Copy)]
pub struct RegisterMapping {
    /// Bytecode register index
    pub bytecode_reg: u8,
    /// Native stack slot or register
    pub native_slot: NativeSlot,
}

/// Location in native code
#[derive(Debug, Clone, Copy)]
pub enum NativeSlot {
    /// CPU register (index depends on calling convention)
    Register(u8),
    /// Stack slot (offset from frame pointer)
    Stack(i32),
}

/// Deoptimization point - where we can bail out from JIT to interpreter
#[derive(Debug, Clone)]
pub struct DeoptPoint {
    /// Native code offset where deopt can occur
    pub native_offset: usize,
    /// Bytecode offset to resume at
    pub bytecode_offset: usize,
    /// Register state reconstruction info
    pub register_map: Vec<RegisterMapping>,
    /// Reason for potential deoptimization
    pub reason: DeoptReason,
    /// Guard ID for invalidation
    pub guard_id: u32,
}

/// Reasons for deoptimization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeoptReason {
    /// Type assumption was wrong
    TypeMismatch,
    /// Object shape changed
    ShapeChange,
    /// Array bounds check failed
    BoundsCheck,
    /// Division by zero
    DivisionByZero,
    /// Stack overflow
    StackOverflow,
    /// Called function was redefined
    FunctionRedefined,
    /// Explicit deopt (debugging)
    Explicit,
    /// Unknown reason
    Unknown,
}

impl DeoptReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeoptReason::TypeMismatch => "type_mismatch",
            DeoptReason::ShapeChange => "shape_change",
            DeoptReason::BoundsCheck => "bounds_check",
            DeoptReason::DivisionByZero => "division_by_zero",
            DeoptReason::StackOverflow => "stack_overflow",
            DeoptReason::FunctionRedefined => "function_redefined",
            DeoptReason::Explicit => "explicit",
            DeoptReason::Unknown => "unknown",
        }
    }
}

/// Guard check for type assumptions
#[derive(Debug, Clone)]
pub struct TypeGuard {
    /// Unique guard ID
    pub id: u32,
    /// Register being guarded
    pub register: u8,
    /// Expected type
    pub expected_type: GuardedType,
    /// Whether this guard has ever failed
    pub has_failed: bool,
    /// Failure count
    pub failure_count: u32,
}

/// Types that can be guarded
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardedType {
    Int,
    Float,
    Bool,
    String,
    Array,
    Table,
    Function,
    Nil,
    /// Any type (no guard needed)
    Any,
}

impl TypeGuard {
    pub fn new(id: u32, register: u8, expected_type: GuardedType) -> Self {
        Self {
            id,
            register,
            expected_type,
            has_failed: false,
            failure_count: 0,
        }
    }

    /// Record a guard failure
    pub fn fail(&mut self) {
        self.has_failed = true;
        self.failure_count += 1;
    }

    /// Check if guard is stable (rarely fails)
    pub fn is_stable(&self) -> bool {
        self.failure_count < 3
    }
}

/// OSR manager - coordinates on-stack replacement
pub struct OSRManager {
    /// Entry points by (function_id, bytecode_offset)
    entry_points: FnvHashMap<(usize, usize), OSREntryPoint>,
    /// Deopt points by native offset
    deopt_points: FnvHashMap<usize, DeoptPoint>,
    /// Type guards by ID
    guards: FnvHashMap<u32, TypeGuard>,
    /// Next guard ID
    next_guard_id: u32,
    /// Statistics
    pub stats: OSRStats,
}

#[derive(Debug, Default, Clone)]
pub struct OSRStats {
    /// Number of OSR entries (interpreter -> JIT)
    pub osr_entries: u64,
    /// Number of deoptimizations (JIT -> interpreter)
    pub deoptimizations: u64,
    /// Deopt counts by reason
    pub deopt_reasons: HashMap<DeoptReason, u64>,
    /// Guard failures
    pub guard_failures: u64,
    /// Guard checks passed
    pub guard_passes: u64,
}

impl OSRManager {
    pub fn new() -> Self {
        Self {
            entry_points: FnvHashMap::default(),
            deopt_points: FnvHashMap::default(),
            guards: FnvHashMap::default(),
            next_guard_id: 1,
            stats: OSRStats::default(),
        }
    }

    /// Register an OSR entry point
    pub fn register_entry_point(
        &mut self,
        func_id: usize,
        entry_point: OSREntryPoint,
    ) {
        let key = (func_id, entry_point.bytecode_offset);
        self.entry_points.insert(key, entry_point);
    }

    /// Register a deoptimization point
    pub fn register_deopt_point(&mut self, deopt_point: DeoptPoint) {
        self.deopt_points.insert(deopt_point.native_offset, deopt_point);
    }

    /// Create a new type guard
    pub fn create_guard(&mut self, register: u8, expected_type: GuardedType) -> u32 {
        let id = self.next_guard_id;
        self.next_guard_id += 1;
        
        let guard = TypeGuard::new(id, register, expected_type);
        self.guards.insert(id, guard);
        
        id
    }

    /// Check if OSR entry is available at a bytecode location
    pub fn can_enter_osr(&self, func_id: usize, bytecode_offset: usize) -> bool {
        self.entry_points.contains_key(&(func_id, bytecode_offset))
    }

    /// Get OSR entry point
    pub fn get_entry_point(
        &self,
        func_id: usize,
        bytecode_offset: usize,
    ) -> Option<&OSREntryPoint> {
        self.entry_points.get(&(func_id, bytecode_offset))
    }

    /// Get deoptimization point for a native offset
    pub fn get_deopt_point(&self, native_offset: usize) -> Option<&DeoptPoint> {
        self.deopt_points.get(&native_offset)
    }

    /// Record an OSR entry
    pub fn record_osr_entry(&mut self) {
        self.stats.osr_entries += 1;
    }

    /// Record a deoptimization
    pub fn record_deoptimization(&mut self, reason: DeoptReason) {
        self.stats.deoptimizations += 1;
        *self.stats.deopt_reasons.entry(reason).or_insert(0) += 1;
    }

    /// Check a type guard
    pub fn check_guard(&mut self, guard_id: u32, actual_type: GuardedType) -> bool {
        if let Some(guard) = self.guards.get_mut(&guard_id) {
            if guard.expected_type == GuardedType::Any || guard.expected_type == actual_type {
                self.stats.guard_passes += 1;
                true
            } else {
                guard.fail();
                self.stats.guard_failures += 1;
                false
            }
        } else {
            // Unknown guard - pass
            true
        }
    }

    /// Get a guard by ID
    pub fn get_guard(&self, guard_id: u32) -> Option<&TypeGuard> {
        self.guards.get(&guard_id)
    }

    /// Check if a guard is stable
    pub fn is_guard_stable(&self, guard_id: u32) -> bool {
        self.guards.get(&guard_id).map(|g| g.is_stable()).unwrap_or(true)
    }

    /// Invalidate all guards for a function (e.g., when function is redefined)
    pub fn invalidate_function(&mut self, func_id: usize) {
        // Remove entry points for this function
        self.entry_points.retain(|(fid, _), _| *fid != func_id);
        
        // Mark deopt points as invalid (would need more tracking in real impl)
    }

    /// Clear all OSR data
    pub fn clear(&mut self) {
        self.entry_points.clear();
        self.deopt_points.clear();
        self.guards.clear();
        self.next_guard_id = 1;
    }

    /// Get statistics
    pub fn stats(&self) -> &OSRStats {
        &self.stats
    }

    /// Get deoptimization rate
    pub fn deopt_rate(&self) -> f64 {
        let total = self.stats.osr_entries + self.stats.deoptimizations;
        if total == 0 {
            0.0
        } else {
            self.stats.deoptimizations as f64 / total as f64
        }
    }

    /// Get guard failure rate
    pub fn guard_failure_rate(&self) -> f64 {
        let total = self.stats.guard_passes + self.stats.guard_failures;
        if total == 0 {
            0.0
        } else {
            self.stats.guard_failures as f64 / total as f64
        }
    }
}

impl Default for OSRManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Deoptimization frame - reconstructed interpreter state
#[derive(Debug, Clone)]
pub struct DeoptFrame {
    /// Function index
    pub func_id: usize,
    /// Bytecode offset to resume at
    pub bytecode_offset: usize,
    /// Register values (indexed by register number)
    pub registers: Vec<i64>,
    /// Return address (for multi-frame deopt)
    pub return_addr: Option<usize>,
}

impl DeoptFrame {
    pub fn new(func_id: usize, bytecode_offset: usize, num_registers: usize) -> Self {
        Self {
            func_id,
            bytecode_offset,
            registers: vec![0; num_registers],
            return_addr: None,
        }
    }

    /// Set a register value
    pub fn set_register(&mut self, reg: u8, value: i64) {
        if (reg as usize) < self.registers.len() {
            self.registers[reg as usize] = value;
        }
    }

    /// Get a register value
    pub fn get_register(&self, reg: u8) -> i64 {
        self.registers.get(reg as usize).copied().unwrap_or(0)
    }
}

/// Deoptimization buffer - holds state during deoptimization
pub struct DeoptBuffer {
    /// Frames to reconstruct (innermost first)
    frames: Vec<DeoptFrame>,
    /// Reason for deoptimization
    reason: DeoptReason,
    /// Native offset where deopt occurred
    native_offset: usize,
}

impl DeoptBuffer {
    pub fn new(reason: DeoptReason, native_offset: usize) -> Self {
        Self {
            frames: Vec::new(),
            reason,
            native_offset,
        }
    }

    /// Add a frame to reconstruct
    pub fn push_frame(&mut self, frame: DeoptFrame) {
        self.frames.push(frame);
    }

    /// Get frames (innermost first)
    pub fn frames(&self) -> &[DeoptFrame] {
        &self.frames
    }

    /// Get reason
    pub fn reason(&self) -> DeoptReason {
        self.reason
    }

    /// Get number of frames
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_osr_entry_point() {
        let entry = OSREntryPoint {
            bytecode_offset: 100,
            native_offset: 0x1000,
            register_map: vec![
                RegisterMapping {
                    bytecode_reg: 0,
                    native_slot: NativeSlot::Register(0),
                },
            ],
            loop_depth: 1,
        };
        
        assert_eq!(entry.bytecode_offset, 100);
        assert_eq!(entry.loop_depth, 1);
    }

    #[test]
    fn test_osr_manager() {
        let mut mgr = OSRManager::new();
        
        let entry = OSREntryPoint {
            bytecode_offset: 50,
            native_offset: 0x2000,
            register_map: vec![],
            loop_depth: 1,
        };
        
        mgr.register_entry_point(0, entry);
        
        assert!(mgr.can_enter_osr(0, 50));
        assert!(!mgr.can_enter_osr(0, 100));
        assert!(!mgr.can_enter_osr(1, 50));
    }

    #[test]
    fn test_type_guard() {
        let mut mgr = OSRManager::new();
        
        let guard_id = mgr.create_guard(0, GuardedType::Int);
        
        assert!(mgr.check_guard(guard_id, GuardedType::Int));
        assert!(!mgr.check_guard(guard_id, GuardedType::String));
        
        assert_eq!(mgr.stats.guard_passes, 1);
        assert_eq!(mgr.stats.guard_failures, 1);
    }

    #[test]
    fn test_guard_stability() {
        let mut mgr = OSRManager::new();
        
        let guard_id = mgr.create_guard(0, GuardedType::Int);
        
        assert!(mgr.is_guard_stable(guard_id));
        
        // Fail 3 times
        mgr.check_guard(guard_id, GuardedType::String);
        mgr.check_guard(guard_id, GuardedType::String);
        mgr.check_guard(guard_id, GuardedType::String);
        
        assert!(!mgr.is_guard_stable(guard_id));
    }

    #[test]
    fn test_deopt_point() {
        let mut mgr = OSRManager::new();
        
        let deopt = DeoptPoint {
            native_offset: 0x3000,
            bytecode_offset: 200,
            register_map: vec![],
            reason: DeoptReason::TypeMismatch,
            guard_id: 0,
        };
        
        mgr.register_deopt_point(deopt);
        
        let found = mgr.get_deopt_point(0x3000);
        assert!(found.is_some());
        assert_eq!(found.unwrap().bytecode_offset, 200);
    }

    #[test]
    fn test_deopt_frame() {
        let mut frame = DeoptFrame::new(0, 100, 8);
        
        frame.set_register(0, 42);
        frame.set_register(3, 100);
        
        assert_eq!(frame.get_register(0), 42);
        assert_eq!(frame.get_register(3), 100);
        assert_eq!(frame.get_register(5), 0);
    }

    #[test]
    fn test_deopt_buffer() {
        let mut buffer = DeoptBuffer::new(DeoptReason::TypeMismatch, 0x1000);
        
        buffer.push_frame(DeoptFrame::new(0, 50, 4));
        buffer.push_frame(DeoptFrame::new(1, 100, 4));
        
        assert_eq!(buffer.frame_count(), 2);
        assert_eq!(buffer.reason(), DeoptReason::TypeMismatch);
    }

    #[test]
    fn test_deopt_stats() {
        let mut mgr = OSRManager::new();
        
        mgr.record_osr_entry();
        mgr.record_osr_entry();
        mgr.record_deoptimization(DeoptReason::TypeMismatch);
        
        assert_eq!(mgr.stats.osr_entries, 2);
        assert_eq!(mgr.stats.deoptimizations, 1);
        assert_eq!(*mgr.stats.deopt_reasons.get(&DeoptReason::TypeMismatch).unwrap(), 1);
    }

    #[test]
    fn test_invalidate_function() {
        let mut mgr = OSRManager::new();
        
        mgr.register_entry_point(0, OSREntryPoint {
            bytecode_offset: 10,
            native_offset: 0x1000,
            register_map: vec![],
            loop_depth: 1,
        });
        mgr.register_entry_point(0, OSREntryPoint {
            bytecode_offset: 20,
            native_offset: 0x2000,
            register_map: vec![],
            loop_depth: 2,
        });
        mgr.register_entry_point(1, OSREntryPoint {
            bytecode_offset: 10,
            native_offset: 0x3000,
            register_map: vec![],
            loop_depth: 1,
        });
        
        assert!(mgr.can_enter_osr(0, 10));
        assert!(mgr.can_enter_osr(0, 20));
        assert!(mgr.can_enter_osr(1, 10));
        
        mgr.invalidate_function(0);
        
        assert!(!mgr.can_enter_osr(0, 10));
        assert!(!mgr.can_enter_osr(0, 20));
        assert!(mgr.can_enter_osr(1, 10)); // Function 1 still valid
    }
}
