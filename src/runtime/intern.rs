//! String interning for deduplicated strings

use std::hash::{Hash, Hasher};
use std::rc::Rc;
use fnv::FnvHashMap;

/// An interned string - cheap to clone and compare
#[derive(Clone)]
pub struct InternedString {
    inner: Rc<str>,
}

impl InternedString {
    /// Get the string content
    pub fn as_str(&self) -> &str {
        &self.inner
    }

    /// Get the length
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get raw pointer for identity comparison
    pub fn as_ptr(&self) -> *const str {
        Rc::as_ptr(&self.inner)
    }
}

impl PartialEq for InternedString {
    fn eq(&self, other: &Self) -> bool {
        // Fast path: pointer equality
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for InternedString {}

impl Hash for InternedString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash by pointer for speed (all equal strings share pointer)
        Rc::as_ptr(&self.inner).hash(state);
    }
}

impl std::fmt::Debug for InternedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.inner)
    }
}

impl std::fmt::Display for InternedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl AsRef<str> for InternedString {
    fn as_ref(&self) -> &str {
        &self.inner
    }
}

impl std::ops::Deref for InternedString {
    type Target = str;
    
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// String interner - maintains a pool of deduplicated strings
pub struct StringInterner {
    /// Map from string content to interned reference
    pool: FnvHashMap<u64, Vec<InternedString>>,
    
    /// Statistics
    pub stats: InternerStats,
}

#[derive(Debug, Default, Clone)]
pub struct InternerStats {
    /// Number of unique strings
    pub unique_count: usize,
    /// Number of intern requests
    pub intern_calls: u64,
    /// Number of cache hits
    pub cache_hits: u64,
    /// Total bytes saved by deduplication
    pub bytes_saved: u64,
}

impl StringInterner {
    pub fn new() -> Self {
        Self {
            pool: FnvHashMap::default(),
            stats: InternerStats::default(),
        }
    }

    /// Intern a string, returning the interned reference
    pub fn intern(&mut self, s: &str) -> InternedString {
        self.stats.intern_calls += 1;
        
        // Hash the string
        let hash = Self::hash_str(s);
        
        // Check if already interned
        if let Some(bucket) = self.pool.get(&hash) {
            for interned in bucket {
                if interned.as_str() == s {
                    self.stats.cache_hits += 1;
                    self.stats.bytes_saved += s.len() as u64;
                    return interned.clone();
                }
            }
        }
        
        // Not found, create new interned string
        let interned = InternedString {
            inner: Rc::from(s),
        };
        
        self.pool
            .entry(hash)
            .or_insert_with(Vec::new)
            .push(interned.clone());
        
        self.stats.unique_count += 1;
        interned
    }

    /// Intern an owned string
    pub fn intern_owned(&mut self, s: String) -> InternedString {
        self.intern(&s)
    }

    /// Check if a string is already interned
    pub fn is_interned(&self, s: &str) -> bool {
        let hash = Self::hash_str(s);
        if let Some(bucket) = self.pool.get(&hash) {
            bucket.iter().any(|interned| interned.as_str() == s)
        } else {
            false
        }
    }

    /// Get the number of unique strings
    pub fn len(&self) -> usize {
        self.stats.unique_count
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.stats.unique_count == 0
    }

    /// Clear all interned strings
    pub fn clear(&mut self) {
        self.pool.clear();
        self.stats.unique_count = 0;
    }

    /// Hash a string using FNV-1a
    fn hash_str(s: &str) -> u64 {
        use std::hash::Hasher;
        let mut hasher = fnv::FnvHasher::default();
        s.hash(&mut hasher);
        hasher.finish()
    }

    /// Get statistics
    pub fn stats(&self) -> &InternerStats {
        &self.stats
    }

    /// Get cache hit rate
    pub fn hit_rate(&self) -> f64 {
        if self.stats.intern_calls == 0 {
            0.0
        } else {
            self.stats.cache_hits as f64 / self.stats.intern_calls as f64
        }
    }
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::new()
    }
}

/// Global string interner (thread-local for safety)
thread_local! {
    static INTERNER: std::cell::RefCell<StringInterner> = std::cell::RefCell::new(StringInterner::new());
}

/// Intern a string using the global interner
pub fn intern(s: &str) -> InternedString {
    INTERNER.with(|interner| interner.borrow_mut().intern(s))
}

/// Intern an owned string using the global interner
pub fn intern_owned(s: String) -> InternedString {
    INTERNER.with(|interner| interner.borrow_mut().intern_owned(s))
}

/// Get global interner statistics
pub fn interner_stats() -> InternerStats {
    INTERNER.with(|interner| interner.borrow().stats.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intern_returns_same() {
        let mut interner = StringInterner::new();
        let s1 = interner.intern("hello");
        let s2 = interner.intern("hello");
        
        // Should be the same pointer
        assert!(Rc::ptr_eq(&s1.inner, &s2.inner));
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_different_strings() {
        let mut interner = StringInterner::new();
        let s1 = interner.intern("hello");
        let s2 = interner.intern("world");
        
        assert_ne!(s1, s2);
        assert!(!Rc::ptr_eq(&s1.inner, &s2.inner));
    }

    #[test]
    fn test_intern_owned() {
        let mut interner = StringInterner::new();
        let s1 = interner.intern("hello");
        let s2 = interner.intern_owned("hello".to_string());
        
        assert_eq!(s1, s2);
        assert!(Rc::ptr_eq(&s1.inner, &s2.inner));
    }

    #[test]
    fn test_stats() {
        let mut interner = StringInterner::new();
        interner.intern("hello");
        interner.intern("hello");
        interner.intern("world");
        
        assert_eq!(interner.stats.unique_count, 2);
        assert_eq!(interner.stats.intern_calls, 3);
        assert_eq!(interner.stats.cache_hits, 1);
    }

    #[test]
    fn test_is_interned() {
        let mut interner = StringInterner::new();
        interner.intern("hello");
        
        assert!(interner.is_interned("hello"));
        assert!(!interner.is_interned("world"));
    }

    #[test]
    fn test_hash_equality() {
        let mut interner = StringInterner::new();
        let s1 = interner.intern("hello");
        let s2 = interner.intern("hello");
        
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(s1.clone());
        
        assert!(set.contains(&s2));
    }

    #[test]
    fn test_display() {
        let mut interner = StringInterner::new();
        let s = interner.intern("hello");
        
        assert_eq!(format!("{}", s), "hello");
        assert_eq!(s.as_str(), "hello");
    }

    #[test]
    fn test_global_intern() {
        let s1 = intern("global_test");
        let s2 = intern("global_test");
        
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_hit_rate() {
        let mut interner = StringInterner::new();
        interner.intern("a");
        interner.intern("a");
        interner.intern("a");
        interner.intern("a");
        
        // 4 calls, 3 hits
        assert!((interner.hit_rate() - 0.75).abs() < 0.001);
    }
}
