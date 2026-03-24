//! Hot path profiling for JIT compilation

use fnv::FnvHashMap;
use std::collections::VecDeque;

/// Default threshold for considering a function hot
pub const DEFAULT_HOT_THRESHOLD: u32 = 100;

/// Default threshold for considering a loop hot
pub const DEFAULT_LOOP_THRESHOLD: u32 = 50;

/// Type profile entry for a value slot
#[derive(Debug, Clone, Default)]
pub struct TypeProfile {
    pub int_count: u32,
    pub float_count: u32,
    pub string_count: u32,
    pub other_count: u32,
}

impl TypeProfile {
    pub fn record_int(&mut self) {
        self.int_count = self.int_count.saturating_add(1);
    }

    pub fn record_float(&mut self) {
        self.float_count = self.float_count.saturating_add(1);
    }

    pub fn record_string(&mut self) {
        self.string_count = self.string_count.saturating_add(1);
    }

    pub fn record_other(&mut self) {
        self.other_count = self.other_count.saturating_add(1);
    }

    pub fn total(&self) -> u32 {
        self.int_count + self.float_count + self.string_count + self.other_count
    }

    /// Returns the dominant type if one exists (>80% of samples)
    pub fn dominant_type(&self) -> Option<DominantType> {
        let total = self.total();
        if total < 10 {
            return None;
        }

        let threshold = (total as f64 * 0.8) as u32;

        if self.int_count >= threshold {
            Some(DominantType::Int)
        } else if self.float_count >= threshold {
            Some(DominantType::Float)
        } else if self.string_count >= threshold {
            Some(DominantType::String)
        } else {
            None
        }
    }
}

/// Dominant type hint for specialization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DominantType {
    Int,
    Float,
    String,
}

/// Profile data for a single function
#[derive(Debug, Clone, Default)]
pub struct FunctionProfile {
    /// Number of times this function was called
    pub call_count: u32,

    /// Number of times each loop back-edge was taken
    pub loop_counts: FnvHashMap<usize, u32>,

    /// Type profiles for argument slots
    pub arg_types: Vec<TypeProfile>,

    /// Type profiles for local variable slots
    pub local_types: FnvHashMap<u8, TypeProfile>,

    /// Whether this function has been JIT compiled
    pub is_compiled: bool,

    /// Whether compilation failed (don't retry)
    pub compile_failed: bool,
}

impl FunctionProfile {
    pub fn new(arity: u8) -> Self {
        Self {
            call_count: 0,
            loop_counts: FnvHashMap::default(),
            arg_types: vec![TypeProfile::default(); arity as usize],
            local_types: FnvHashMap::default(),
            is_compiled: false,
            compile_failed: false,
        }
    }

    pub fn record_call(&mut self) {
        self.call_count = self.call_count.saturating_add(1);
    }

    pub fn record_loop(&mut self, loop_offset: usize) {
        *self.loop_counts.entry(loop_offset).or_insert(0) += 1;
    }

    pub fn is_hot(&self, threshold: u32) -> bool {
        self.call_count >= threshold
    }

    pub fn has_hot_loop(&self, threshold: u32) -> bool {
        self.loop_counts.values().any(|&count| count >= threshold)
    }

    pub fn hottest_loop(&self) -> Option<(usize, u32)> {
        self.loop_counts
            .iter()
            .max_by_key(|(_, &count)| count)
            .map(|(&offset, &count)| (offset, count))
    }
}

/// JIT profiler for identifying hot paths
pub struct Profiler {
    /// Per-function profiles indexed by function ID
    profiles: FnvHashMap<usize, FunctionProfile>,

    /// Threshold for function hotness
    pub hot_threshold: u32,

    /// Threshold for loop hotness
    pub loop_threshold: u32,

    /// Queue of functions ready for JIT compilation
    compile_queue: VecDeque<usize>,

    /// Statistics
    pub stats: ProfilerStats,
}

#[derive(Debug, Clone, Default)]
pub struct ProfilerStats {
    pub total_calls: u64,
    pub total_loop_iterations: u64,
    pub functions_profiled: usize,
    pub functions_compiled: usize,
    pub compilation_failures: usize,
}

impl Profiler {
    pub fn new() -> Self {
        Self {
            profiles: FnvHashMap::default(),
            hot_threshold: DEFAULT_HOT_THRESHOLD,
            loop_threshold: DEFAULT_LOOP_THRESHOLD,
            compile_queue: VecDeque::new(),
            stats: ProfilerStats::default(),
        }
    }

    pub fn with_thresholds(hot_threshold: u32, loop_threshold: u32) -> Self {
        Self {
            profiles: FnvHashMap::default(),
            hot_threshold,
            loop_threshold,
            compile_queue: VecDeque::new(),
            stats: ProfilerStats::default(),
        }
    }

    /// Initialize profile for a function
    pub fn init_function(&mut self, func_id: usize, arity: u8) {
        if !self.profiles.contains_key(&func_id) {
            self.profiles.insert(func_id, FunctionProfile::new(arity));
            self.stats.functions_profiled += 1;
        }
    }

    /// Record a function call
    pub fn record_call(&mut self, func_id: usize) {
        self.stats.total_calls += 1;

        if let Some(profile) = self.profiles.get_mut(&func_id) {
            profile.record_call();

            // Check if function just became hot
            if !profile.is_compiled
                && !profile.compile_failed
                && profile.is_hot(self.hot_threshold)
            {
                self.compile_queue.push_back(func_id);
            }
        }
    }

    /// Record a loop iteration
    pub fn record_loop(&mut self, func_id: usize, loop_offset: usize) {
        self.stats.total_loop_iterations += 1;

        if let Some(profile) = self.profiles.get_mut(&func_id) {
            profile.record_loop(loop_offset);
        }
    }

    /// Record argument types for a function call
    pub fn record_arg_type(&mut self, func_id: usize, arg_index: usize, type_tag: TypeTag) {
        if let Some(profile) = self.profiles.get_mut(&func_id) {
            if let Some(arg_profile) = profile.arg_types.get_mut(arg_index) {
                match type_tag {
                    TypeTag::Int => arg_profile.record_int(),
                    TypeTag::Float => arg_profile.record_float(),
                    TypeTag::String => arg_profile.record_string(),
                    _ => arg_profile.record_other(),
                }
            }
        }
    }

    /// Record local variable type
    pub fn record_local_type(&mut self, func_id: usize, slot: u8, type_tag: TypeTag) {
        if let Some(profile) = self.profiles.get_mut(&func_id) {
            let local_profile = profile.local_types.entry(slot).or_default();
            match type_tag {
                TypeTag::Int => local_profile.record_int(),
                TypeTag::Float => local_profile.record_float(),
                TypeTag::String => local_profile.record_string(),
                _ => local_profile.record_other(),
            }
        }
    }

    /// Check if a function is hot
    pub fn is_hot(&self, func_id: usize) -> bool {
        self.profiles
            .get(&func_id)
            .map(|p| p.is_hot(self.hot_threshold))
            .unwrap_or(false)
    }

    /// Check if a function has a hot loop
    pub fn has_hot_loop(&self, func_id: usize) -> bool {
        self.profiles
            .get(&func_id)
            .map(|p| p.has_hot_loop(self.loop_threshold))
            .unwrap_or(false)
    }

    /// Get the next function to compile, if any
    pub fn next_compile_candidate(&mut self) -> Option<usize> {
        while let Some(func_id) = self.compile_queue.pop_front() {
            if let Some(profile) = self.profiles.get(&func_id) {
                if !profile.is_compiled && !profile.compile_failed {
                    return Some(func_id);
                }
            }
        }
        None
    }

    /// Mark a function as compiled
    pub fn mark_compiled(&mut self, func_id: usize) {
        if let Some(profile) = self.profiles.get_mut(&func_id) {
            profile.is_compiled = true;
            self.stats.functions_compiled += 1;
        }
    }

    /// Mark a function compilation as failed
    pub fn mark_compile_failed(&mut self, func_id: usize) {
        if let Some(profile) = self.profiles.get_mut(&func_id) {
            profile.compile_failed = true;
            self.stats.compilation_failures += 1;
        }
    }

    /// Get profile for a function
    pub fn get_profile(&self, func_id: usize) -> Option<&FunctionProfile> {
        self.profiles.get(&func_id)
    }

    /// Get call count for a function
    pub fn get_call_count(&self, func_id: usize) -> u32 {
        self.profiles.get(&func_id).map(|p| p.call_count).unwrap_or(0)
    }

    /// Reset all profiles
    pub fn reset(&mut self) {
        self.profiles.clear();
        self.compile_queue.clear();
        self.stats = ProfilerStats::default();
    }
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple type tag for profiling (avoiding full Value dependency)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeTag {
    Nil,
    Bool,
    Int,
    Float,
    String,
    Array,
    Table,
    Function,
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profiler_new() {
        let profiler = Profiler::new();
        assert_eq!(profiler.hot_threshold, DEFAULT_HOT_THRESHOLD);
        assert_eq!(profiler.stats.total_calls, 0);
    }

    #[test]
    fn test_function_becomes_hot() {
        let mut profiler = Profiler::with_thresholds(10, 5);
        profiler.init_function(0, 2);

        for _ in 0..9 {
            profiler.record_call(0);
        }
        assert!(!profiler.is_hot(0));

        profiler.record_call(0);
        assert!(profiler.is_hot(0));
    }

    #[test]
    fn test_compile_queue() {
        let mut profiler = Profiler::with_thresholds(5, 5);
        profiler.init_function(0, 0);

        // Not hot yet
        assert!(profiler.next_compile_candidate().is_none());

        // Make it hot
        for _ in 0..5 {
            profiler.record_call(0);
        }

        assert_eq!(profiler.next_compile_candidate(), Some(0));
        assert!(profiler.next_compile_candidate().is_none()); // Consumed

        // Mark as compiled
        profiler.mark_compiled(0);
        assert!(profiler.profiles.get(&0).unwrap().is_compiled);
    }

    #[test]
    fn test_loop_profiling() {
        let mut profiler = Profiler::with_thresholds(100, 10);
        profiler.init_function(0, 0);

        for _ in 0..9 {
            profiler.record_loop(0, 42);
        }
        assert!(!profiler.has_hot_loop(0));

        profiler.record_loop(0, 42);
        assert!(profiler.has_hot_loop(0));

        let profile = profiler.get_profile(0).unwrap();
        assert_eq!(profile.hottest_loop(), Some((42, 10)));
    }

    #[test]
    fn test_type_profile_dominant() {
        let mut tp = TypeProfile::default();

        // Not enough samples
        for _ in 0..5 {
            tp.record_int();
        }
        assert!(tp.dominant_type().is_none());

        // Now enough, and int is dominant
        for _ in 0..10 {
            tp.record_int();
        }
        assert_eq!(tp.dominant_type(), Some(DominantType::Int));

        // Add other types, int no longer dominant
        for _ in 0..5 {
            tp.record_float();
        }
        assert!(tp.dominant_type().is_none());
    }

    #[test]
    fn test_arg_type_profiling() {
        let mut profiler = Profiler::new();
        profiler.init_function(0, 2);

        for _ in 0..20 {
            profiler.record_arg_type(0, 0, TypeTag::Int);
            profiler.record_arg_type(0, 1, TypeTag::String);
        }

        let profile = profiler.get_profile(0).unwrap();
        assert_eq!(profile.arg_types[0].dominant_type(), Some(DominantType::Int));
        assert_eq!(profile.arg_types[1].dominant_type(), Some(DominantType::String));
    }
}
