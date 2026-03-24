//! Garbage collector

pub mod heap;

/// Garbage collector statistics
#[derive(Debug, Default)]
pub struct GCStats {
    pub collections: u64,
    pub bytes_allocated: usize,
    pub bytes_freed: usize,
}

/// The garbage collector
pub struct GC {
    stats: GCStats,
}

impl GC {
    pub fn new() -> Self {
        Self {
            stats: GCStats::default(),
        }
    }

    pub fn stats(&self) -> &GCStats {
        &self.stats
    }
}

impl Default for GC {
    fn default() -> Self {
        Self::new()
    }
}
