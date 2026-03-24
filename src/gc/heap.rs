//! Heap allocation for GC-managed objects

/// A managed heap for GC objects
pub struct Heap {
    max_size: usize,
    allocated: usize,
}

impl Heap {
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size,
            allocated: 0,
        }
    }

    pub fn allocated(&self) -> usize {
        self.allocated
    }

    pub fn max_size(&self) -> usize {
        self.max_size
    }
}

impl Default for Heap {
    fn default() -> Self {
        Self::new(64 * 1024 * 1024) // 64MB default
    }
}
