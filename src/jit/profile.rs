//! Hot path profiling for JIT

use fnv::FnvHashMap;

/// Default threshold for considering a function hot
const HOT_THRESHOLD: u32 = 100;

/// Profiler for identifying hot paths
pub struct Profiler {
    call_counts: FnvHashMap<usize, u32>,
    threshold: u32,
}

impl Profiler {
    pub fn new() -> Self {
        Self {
            call_counts: FnvHashMap::default(),
            threshold: HOT_THRESHOLD,
        }
    }

    pub fn record_call(&mut self, func_id: usize) {
        *self.call_counts.entry(func_id).or_insert(0) += 1;
    }

    pub fn is_hot(&self, func_id: usize) -> bool {
        self.call_counts
            .get(&func_id)
            .map(|&count| count >= self.threshold)
            .unwrap_or(false)
    }

    pub fn get_count(&self, func_id: usize) -> u32 {
        self.call_counts.get(&func_id).copied().unwrap_or(0)
    }
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}
