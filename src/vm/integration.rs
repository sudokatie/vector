//! VM integration with advanced features
//!
//! This module wires together:
//! - Generational GC with write barriers
//! - Inline caching for property access
//! - On-stack replacement (OSR) for hot loops
//! - Deoptimization when guards fail

use crate::gc::{GenerationalGC, WriteBarrier, GenGCStats};
use crate::jit::{
    InlineCacheManager, ICStats, ShapeId, PropertySlot, AccessSite,
    OSRManager, OSRStats, DeoptReason, GuardedType, DeoptFrame,
};
use crate::jit::profile::FunctionProfile;
use std::collections::HashMap;

/// Integrated VM runtime with all advanced features
pub struct IntegratedRuntime {
    /// Generational garbage collector
    pub gc: GenerationalGC,
    
    /// Inline cache manager for property access
    pub ic_manager: InlineCacheManager,
    
    /// OSR manager for on-stack replacement
    pub osr_manager: OSRManager,
    
    /// Object shapes for inline caching
    shapes: HashMap<u64, ShapeId>,
    
    /// Whether advanced features are enabled
    features_enabled: AdvancedFeatures,
    
    /// Statistics
    pub stats: IntegrationStats,
}

#[derive(Debug, Clone, Copy)]
pub struct AdvancedFeatures {
    pub generational_gc: bool,
    pub inline_caching: bool,
    pub osr: bool,
    pub deoptimization: bool,
}

impl Default for AdvancedFeatures {
    fn default() -> Self {
        Self {
            generational_gc: true,
            inline_caching: true,
            osr: true,
            deoptimization: true,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct IntegrationStats {
    /// Property accesses
    pub property_accesses: u64,
    /// IC hits
    pub ic_hits: u64,
    /// Shape transitions
    pub shape_transitions: u64,
    /// OSR entries
    pub osr_entries: u64,
    /// Deoptimizations
    pub deopts: u64,
    /// Write barrier invocations
    pub write_barriers: u64,
}

impl IntegratedRuntime {
    /// Create a new integrated runtime
    pub fn new() -> Self {
        Self {
            gc: GenerationalGC::with_defaults(),
            ic_manager: InlineCacheManager::new(),
            osr_manager: OSRManager::new(),
            shapes: HashMap::new(),
            features_enabled: AdvancedFeatures::default(),
            stats: IntegrationStats::default(),
        }
    }

    /// Create with specific feature flags
    pub fn with_features(features: AdvancedFeatures) -> Self {
        Self {
            gc: GenerationalGC::with_defaults(),
            ic_manager: InlineCacheManager::new(),
            osr_manager: OSRManager::new(),
            shapes: HashMap::new(),
            features_enabled: features,
            stats: IntegrationStats::default(),
        }
    }

    /// Disable all advanced features (for debugging)
    pub fn disable_all(&mut self) {
        self.features_enabled = AdvancedFeatures {
            generational_gc: false,
            inline_caching: false,
            osr: false,
            deoptimization: false,
        };
    }

    // === Garbage Collection ===

    /// Check if young generation collection is needed
    pub fn should_collect_young(&self) -> bool {
        self.features_enabled.generational_gc && self.gc.should_collect_young()
    }

    /// Check if full collection is needed
    pub fn should_collect_full(&self) -> bool {
        self.features_enabled.generational_gc && self.gc.should_collect_full()
    }

    /// Perform young generation collection
    pub fn collect_young(&mut self, roots: &mut [*const crate::gc::ObjectHeader]) {
        if self.features_enabled.generational_gc {
            self.gc.collect_young(roots);
        }
    }

    /// Perform full collection
    pub fn collect_full(&mut self, roots: &mut [*const crate::gc::ObjectHeader]) {
        if self.features_enabled.generational_gc {
            self.gc.collect_full(roots);
        }
    }

    /// Record a write (for write barrier)
    pub fn record_write(
        &mut self,
        source: std::ptr::NonNull<crate::gc::ObjectHeader>,
        target: std::ptr::NonNull<crate::gc::ObjectHeader>,
    ) {
        if self.features_enabled.generational_gc {
            self.gc.write_barrier.on_write(source, target);
            self.stats.write_barriers += 1;
        }
    }

    // === Inline Caching ===

    /// Look up a property using inline cache
    pub fn ic_lookup(
        &mut self,
        bytecode_offset: u32,
        property_name: &str,
        object_shape: ShapeId,
    ) -> Option<PropertySlot> {
        if !self.features_enabled.inline_caching {
            return None;
        }

        self.stats.property_accesses += 1;
        let site = AccessSite::new(bytecode_offset, property_name);
        
        if let Some(slot) = self.ic_manager.lookup(site, object_shape) {
            self.stats.ic_hits += 1;
            Some(slot)
        } else {
            None
        }
    }

    /// Record a property access for inline caching
    pub fn ic_record(
        &mut self,
        bytecode_offset: u32,
        property_name: &str,
        object_shape: ShapeId,
        slot: PropertySlot,
    ) {
        if self.features_enabled.inline_caching {
            let site = AccessSite::new(bytecode_offset, property_name);
            self.ic_manager.record(site, object_shape, slot);
        }
    }

    /// Get or create a shape ID for an object
    pub fn get_shape(&mut self, shape_hash: u64) -> ShapeId {
        if let Some(&id) = self.shapes.get(&shape_hash) {
            id
        } else {
            let id = self.ic_manager.new_shape_id();
            self.shapes.insert(shape_hash, id);
            self.stats.shape_transitions += 1;
            id
        }
    }

    // === On-Stack Replacement ===

    /// Check if we can enter JIT code at this bytecode offset
    pub fn can_osr(&self, func_id: usize, bytecode_offset: usize) -> bool {
        self.features_enabled.osr && self.osr_manager.can_enter_osr(func_id, bytecode_offset)
    }

    /// Record an OSR entry
    pub fn record_osr_entry(&mut self) {
        if self.features_enabled.osr {
            self.osr_manager.record_osr_entry();
            self.stats.osr_entries += 1;
        }
    }

    /// Get OSR entry point info
    pub fn get_osr_entry(
        &self,
        func_id: usize,
        bytecode_offset: usize,
    ) -> Option<&crate::jit::OSREntryPoint> {
        if self.features_enabled.osr {
            self.osr_manager.get_entry_point(func_id, bytecode_offset)
        } else {
            None
        }
    }

    // === Deoptimization ===

    /// Check a type guard
    pub fn check_guard(&mut self, guard_id: u32, actual_type: GuardedType) -> bool {
        if !self.features_enabled.deoptimization {
            return true; // No guards when disabled
        }
        self.osr_manager.check_guard(guard_id, actual_type)
    }

    /// Record a deoptimization
    pub fn record_deopt(&mut self, reason: DeoptReason) {
        if self.features_enabled.deoptimization {
            self.osr_manager.record_deoptimization(reason);
            self.stats.deopts += 1;
        }
    }

    /// Invalidate compiled code for a function
    pub fn invalidate_function(&mut self, func_id: usize) {
        self.osr_manager.invalidate_function(func_id);
    }

    // === Type Specialization Support ===

    /// Get the guarded type for a value
    pub fn value_to_guarded_type(type_name: &str) -> GuardedType {
        match type_name {
            "int" => GuardedType::Int,
            "float" => GuardedType::Float,
            "bool" => GuardedType::Bool,
            "string" => GuardedType::String,
            "array" => GuardedType::Array,
            "table" => GuardedType::Table,
            "function" | "closure" => GuardedType::Function,
            "nil" => GuardedType::Nil,
            _ => GuardedType::Any,
        }
    }

    /// Create a type guard for a register
    pub fn create_type_guard(&mut self, register: u8, expected_type: GuardedType) -> u32 {
        if self.features_enabled.deoptimization {
            self.osr_manager.create_guard(register, expected_type)
        } else {
            0 // Dummy guard ID
        }
    }

    // === Statistics ===

    /// Get GC statistics
    pub fn gc_stats(&self) -> &GenGCStats {
        self.gc.stats()
    }

    /// Get IC statistics
    pub fn ic_stats(&self) -> &ICStats {
        self.ic_manager.stats()
    }

    /// Get OSR statistics
    pub fn osr_stats(&self) -> &OSRStats {
        self.osr_manager.stats()
    }

    /// Get integration statistics
    pub fn stats(&self) -> &IntegrationStats {
        &self.stats
    }

    /// Get cache hit rate
    pub fn ic_hit_rate(&self) -> f64 {
        if self.stats.property_accesses == 0 {
            0.0
        } else {
            self.stats.ic_hits as f64 / self.stats.property_accesses as f64
        }
    }

    /// Reset all statistics
    pub fn reset_stats(&mut self) {
        self.stats = IntegrationStats::default();
    }
}

impl Default for IntegratedRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// Type specializer - uses profiling data to specialize JIT code
pub struct TypeSpecializer {
    /// Type profiles per function
    profiles: HashMap<usize, FunctionProfile>,
    /// Specialization threshold (how many samples before specializing)
    threshold: u32,
}

impl TypeSpecializer {
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
            threshold: 100,
        }
    }

    /// Record a type observation
    pub fn record_type(&mut self, func_id: usize, slot: u8, type_tag: crate::jit::TypeTag) {
        let profile = self.profiles
            .entry(func_id)
            .or_insert_with(|| FunctionProfile::new(0));
        
        profile.record_call();
        
        // Record local type
        let local_profile = profile.local_types.entry(slot).or_default();
        match type_tag {
            crate::jit::TypeTag::Int => local_profile.record_int(),
            crate::jit::TypeTag::Float => local_profile.record_float(),
            crate::jit::TypeTag::String => local_profile.record_string(),
            _ => local_profile.record_other(),
        }
    }

    /// Get the dominant type for a slot
    pub fn get_dominant_type(&self, func_id: usize, slot: u8) -> Option<crate::jit::DominantType> {
        self.profiles.get(&func_id)
            .and_then(|p| p.local_types.get(&slot))
            .and_then(|tp| tp.dominant_type())
    }

    /// Check if we have enough samples to specialize
    pub fn can_specialize(&self, func_id: usize) -> bool {
        self.profiles.get(&func_id)
            .map(|p| p.call_count >= self.threshold)
            .unwrap_or(false)
    }

    /// Get function profile
    pub fn get_profile(&self, func_id: usize) -> Option<&FunctionProfile> {
        self.profiles.get(&func_id)
    }
}

impl Default for TypeSpecializer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integrated_runtime_creation() {
        let runtime = IntegratedRuntime::new();
        assert!(runtime.features_enabled.generational_gc);
        assert!(runtime.features_enabled.inline_caching);
    }

    #[test]
    fn test_disable_features() {
        let mut runtime = IntegratedRuntime::new();
        runtime.disable_all();
        assert!(!runtime.features_enabled.generational_gc);
        assert!(!runtime.features_enabled.inline_caching);
    }

    #[test]
    fn test_shape_allocation() {
        let mut runtime = IntegratedRuntime::new();
        
        let shape1 = runtime.get_shape(12345);
        let shape2 = runtime.get_shape(12345);
        let shape3 = runtime.get_shape(67890);
        
        assert_eq!(shape1, shape2); // Same hash = same shape
        assert_ne!(shape1, shape3); // Different hash = different shape
    }

    #[test]
    fn test_type_guard() {
        let mut runtime = IntegratedRuntime::new();
        
        let guard_id = runtime.create_type_guard(0, GuardedType::Int);
        
        assert!(runtime.check_guard(guard_id, GuardedType::Int));
        assert!(!runtime.check_guard(guard_id, GuardedType::String));
    }

    #[test]
    fn test_ic_lookup_miss() {
        let mut runtime = IntegratedRuntime::new();
        let shape = runtime.get_shape(123);
        
        // First lookup should miss
        let result = runtime.ic_lookup(0, "foo", shape);
        assert!(result.is_none());
        
        // Record the access
        runtime.ic_record(0, "foo", shape, PropertySlot { offset: 5, is_own: true });
        
        // Second lookup should hit
        let result = runtime.ic_lookup(0, "foo", shape);
        assert!(result.is_some());
        assert_eq!(result.unwrap().offset, 5);
    }

    #[test]
    fn test_type_specializer() {
        let mut spec = TypeSpecializer::new();
        
        // Record many int observations
        for _ in 0..200 {
            spec.record_type(0, 0, crate::jit::TypeTag::Int);
        }
        
        assert!(spec.can_specialize(0));
        assert_eq!(spec.get_dominant_type(0, 0), Some(crate::jit::DominantType::Int));
    }

    #[test]
    fn test_stats_tracking() {
        let mut runtime = IntegratedRuntime::new();
        let shape = runtime.get_shape(123);
        
        runtime.ic_lookup(0, "test", shape);
        runtime.ic_lookup(0, "test2", shape);
        
        assert_eq!(runtime.stats.property_accesses, 2);
    }
}
