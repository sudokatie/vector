//! Compiled code cache

use super::codegen::CompiledCode;
use fnv::FnvHashMap;

/// Cache for JIT-compiled native code
pub struct CodeCache {
    /// Compiled functions indexed by function ID
    cache: FnvHashMap<usize, CompiledCode>,

    /// Maximum cache size (number of functions)
    max_size: usize,

    /// Statistics
    pub stats: CacheStats,
}

#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub total_code_size: usize,
}

impl CodeCache {
    pub fn new() -> Self {
        Self::with_max_size(1024)
    }

    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            cache: FnvHashMap::default(),
            max_size,
            stats: CacheStats::default(),
        }
    }

    /// Check if a function is in the cache
    pub fn contains(&self, func_id: usize) -> bool {
        self.cache.contains_key(&func_id)
    }

    /// Get compiled code for a function
    pub fn get(&self, func_id: usize) -> Option<&CompiledCode> {
        self.cache.get(&func_id)
    }

    /// Insert compiled code into the cache
    pub fn insert(&mut self, func_id: usize, code: CompiledCode) {
        // Evict if at capacity
        if self.cache.len() >= self.max_size && !self.cache.contains_key(&func_id) {
            self.evict_one();
        }

        self.stats.total_code_size += code.code_size();
        self.cache.insert(func_id, code);
    }

    /// Invalidate compiled code for a function
    pub fn invalidate(&mut self, func_id: usize) {
        if let Some(code) = self.cache.remove(&func_id) {
            self.stats.total_code_size = self.stats.total_code_size.saturating_sub(code.code_size());
        }
    }

    /// Clear the entire cache
    pub fn clear(&mut self) {
        self.cache.clear();
        self.stats.total_code_size = 0;
    }

    /// Get number of cached functions
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Evict one entry (simple strategy: remove first found)
    fn evict_one(&mut self) {
        if let Some(&func_id) = self.cache.keys().next() {
            self.invalidate(func_id);
            self.stats.evictions += 1;
        }
    }

    /// Record a cache hit
    pub fn record_hit(&mut self) {
        self.stats.hits += 1;
    }

    /// Record a cache miss
    pub fn record_miss(&mut self) {
        self.stats.misses += 1;
    }
}

impl Default for CodeCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_insert_get() {
        let mut cache = CodeCache::new();
        let code = CompiledCode::stub(42);

        cache.insert(0, code);
        assert!(cache.contains(0));
        assert!(!cache.contains(1));

        let retrieved = cache.get(0).unwrap();
        assert_eq!(retrieved.func_id, 42);
    }

    #[test]
    fn test_cache_invalidate() {
        let mut cache = CodeCache::new();
        cache.insert(0, CompiledCode::stub(0));
        cache.insert(1, CompiledCode::stub(1));

        assert_eq!(cache.len(), 2);

        cache.invalidate(0);
        assert!(!cache.contains(0));
        assert!(cache.contains(1));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_cache_eviction() {
        let mut cache = CodeCache::with_max_size(2);

        cache.insert(0, CompiledCode::stub(0));
        cache.insert(1, CompiledCode::stub(1));
        assert_eq!(cache.len(), 2);

        // This should trigger eviction
        cache.insert(2, CompiledCode::stub(2));
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.stats.evictions, 1);
    }
}
