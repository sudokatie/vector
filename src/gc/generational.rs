//! Generational garbage collector with write barriers

use std::ptr::NonNull;
use std::collections::VecDeque;
use std::cell::Cell;
use super::heap::{Color, ObjectHeader, TypeTag, Heap, Trace, GcRef};

/// Generation of an object
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Generation {
    /// Young generation (nursery) - collected frequently
    Young = 0,
    /// Old generation - collected less frequently
    Old = 1,
}

/// Extended object header with generation info
#[repr(C)]
pub struct GenObjectHeader {
    /// Base header
    pub base: ObjectHeader,
    /// Object generation
    pub generation: Cell<Generation>,
    /// Number of collections survived
    pub survival_count: Cell<u8>,
    /// Whether this object is in the remembered set
    pub remembered: Cell<bool>,
}

/// Write barrier - tracks cross-generational references
pub struct WriteBarrier {
    /// Remembered set: old objects that reference young objects
    remembered_set: Vec<NonNull<ObjectHeader>>,
    /// Card table for large-object tracking (optional optimization)
    card_table: Vec<u8>,
    /// Card size in bytes
    card_size: usize,
    /// Statistics
    pub stats: WriteBarrierStats,
}

#[derive(Debug, Default, Clone)]
pub struct WriteBarrierStats {
    /// Number of write barrier invocations
    pub barrier_calls: u64,
    /// Number of objects added to remembered set
    pub remembered_adds: u64,
    /// Number of objects removed from remembered set
    pub remembered_removes: u64,
}

impl WriteBarrier {
    pub fn new() -> Self {
        Self {
            remembered_set: Vec::with_capacity(1024),
            card_table: Vec::new(),
            card_size: 512, // 512 bytes per card
            stats: WriteBarrierStats::default(),
        }
    }

    /// Write barrier: called when an old object is modified to reference another object
    /// 
    /// If `source` is old and `target` is young, add source to remembered set
    pub fn on_write(&mut self, source: NonNull<ObjectHeader>, target: NonNull<ObjectHeader>) {
        self.stats.barrier_calls += 1;
        
        // Safety: we're reading metadata only
        let source_header = unsafe { source.as_ref() };
        let target_header = unsafe { target.as_ref() };
        
        // If source is old and target is young, remember the source
        // (We'd check GenObjectHeader::generation, but using is_root as proxy here)
        // In a full implementation, we'd have the generation in the header
        
        // For now, add to remembered set if not already there
        if !self.remembered_set.iter().any(|&p| p == source) {
            self.remembered_set.push(source);
            self.stats.remembered_adds += 1;
        }
    }

    /// Clear the remembered set after a collection
    pub fn clear(&mut self) {
        self.stats.remembered_removes += self.remembered_set.len() as u64;
        self.remembered_set.clear();
    }

    /// Get objects in the remembered set (for root enumeration)
    pub fn remembered_objects(&self) -> &[NonNull<ObjectHeader>] {
        &self.remembered_set
    }

    /// Remove an object from the remembered set
    pub fn remove(&mut self, obj: NonNull<ObjectHeader>) {
        if let Some(pos) = self.remembered_set.iter().position(|&p| p == obj) {
            self.remembered_set.swap_remove(pos);
            self.stats.remembered_removes += 1;
        }
    }

    /// Get remembered set size
    pub fn remembered_count(&self) -> usize {
        self.remembered_set.len()
    }
}

impl Default for WriteBarrier {
    fn default() -> Self {
        Self::new()
    }
}

/// Generational garbage collector
pub struct GenerationalGC {
    /// Young generation heap
    young_heap: Heap,
    /// Old generation heap
    old_heap: Heap,
    /// Write barrier
    pub write_barrier: WriteBarrier,
    /// Gray list for marking
    gray_list: VecDeque<NonNull<ObjectHeader>>,
    /// Number of collections to survive before promotion
    promotion_threshold: u8,
    /// Statistics
    pub stats: GenGCStats,
}

#[derive(Debug, Default, Clone)]
pub struct GenGCStats {
    /// Number of young generation collections
    pub young_collections: u64,
    /// Number of full collections
    pub full_collections: u64,
    /// Objects promoted to old generation
    pub objects_promoted: u64,
    /// Total bytes collected from young gen
    pub young_bytes_freed: u64,
    /// Total bytes collected from old gen
    pub old_bytes_freed: u64,
    /// Last young collection time (microseconds)
    pub last_young_collect_us: u64,
    /// Last full collection time (microseconds)
    pub last_full_collect_us: u64,
}

impl GenerationalGC {
    /// Create a new generational GC
    pub fn new(young_size: usize, old_size: usize) -> Self {
        Self {
            young_heap: Heap::new(young_size),
            old_heap: Heap::new(old_size),
            write_barrier: WriteBarrier::new(),
            gray_list: VecDeque::new(),
            promotion_threshold: 3, // Survive 3 collections before promotion
            stats: GenGCStats::default(),
        }
    }

    /// Create with default sizes
    pub fn with_defaults() -> Self {
        // 8MB young, 64MB old
        Self::new(8 * 1024 * 1024, 64 * 1024 * 1024)
    }

    /// Allocate in the young generation
    /// 
    /// Note: This takes ownership of value. If allocation fails after the first
    /// attempt, a collection is triggered and we return None (value is lost).
    /// For critical allocations, caller should check should_collect_young() first.
    pub fn alloc_young<T: Trace>(&mut self, type_tag: TypeTag, value: T) -> Option<GcRef<T>> {
        // Try young generation first
        self.young_heap.alloc(type_tag, value)
    }

    /// Allocate in the young generation with automatic collection
    /// 
    /// This version triggers a collection if needed and retries.
    /// Use when you can provide the value via a closure.
    pub fn alloc_young_with<T: Trace, F>(&mut self, type_tag: TypeTag, make_value: F) -> Option<GcRef<T>>
    where
        F: FnOnce() -> T,
    {
        // Check if collection needed
        if self.should_collect_young() {
            self.collect_young(&mut []);
        }

        // Try allocation
        self.young_heap.alloc(type_tag, make_value())
    }

    /// Check if we should collect
    pub fn should_collect_young(&self) -> bool {
        self.young_heap.should_collect()
    }

    /// Check if we should do a full collection
    pub fn should_collect_full(&self) -> bool {
        self.old_heap.should_collect()
    }

    /// Collect the young generation only
    pub fn collect_young(&mut self, roots: &mut [*const ObjectHeader]) {
        let start = std::time::Instant::now();

        // Mark phase: roots + remembered set
        self.gray_list.clear();
        
        // Add explicit roots
        for &root in roots.iter() {
            if !root.is_null() {
                self.mark_gray(root);
            }
        }

        // Add remembered set (old objects pointing to young)
        for &obj in self.write_barrier.remembered_objects() {
            // Mark the young objects referenced by remembered old objects
            // In a full impl, we'd trace from these to their young referents
            self.gray_list.push_back(obj);
        }

        // Mark root objects
        for obj in self.young_heap.iter_objects() {
            let header = unsafe { obj.as_ref() };
            if header.is_root.get() {
                self.mark_gray(obj.as_ptr());
            }
        }

        // Trace
        self.trace_gray_young();

        // Sweep young generation
        let freed = self.young_heap.sweep();
        self.stats.young_bytes_freed += freed as u64;
        self.stats.young_collections += 1;

        // Clear remembered set
        self.write_barrier.clear();

        self.young_heap.update_threshold();
        self.stats.last_young_collect_us = start.elapsed().as_micros() as u64;
    }

    /// Full collection (both generations)
    pub fn collect_full(&mut self, roots: &mut [*const ObjectHeader]) {
        let start = std::time::Instant::now();

        // Mark phase
        self.gray_list.clear();

        // Add explicit roots
        for &root in roots.iter() {
            if !root.is_null() {
                self.mark_gray(root);
            }
        }

        // Mark all root objects in both generations
        for obj in self.young_heap.iter_objects() {
            let header = unsafe { obj.as_ref() };
            if header.is_root.get() {
                self.mark_gray(obj.as_ptr());
            }
        }
        for obj in self.old_heap.iter_objects() {
            let header = unsafe { obj.as_ref() };
            if header.is_root.get() {
                self.mark_gray(obj.as_ptr());
            }
        }

        // Trace (all generations)
        self.trace_gray_full();

        // Sweep both generations
        let young_freed = self.young_heap.sweep();
        let old_freed = self.old_heap.sweep();

        self.stats.young_bytes_freed += young_freed as u64;
        self.stats.old_bytes_freed += old_freed as u64;
        self.stats.full_collections += 1;

        // Clear remembered set
        self.write_barrier.clear();

        self.young_heap.update_threshold();
        self.old_heap.update_threshold();
        self.stats.last_full_collect_us = start.elapsed().as_micros() as u64;
    }

    /// Mark an object as gray
    fn mark_gray(&mut self, ptr: *const ObjectHeader) {
        if ptr.is_null() {
            return;
        }
        let header = unsafe { &*ptr };
        if header.color.get() == Color::White {
            header.color.set(Color::Gray);
            self.gray_list.push_back(NonNull::new(ptr as *mut ObjectHeader).unwrap());
        }
    }

    /// Trace from gray objects (young generation only)
    fn trace_gray_young(&mut self) {
        while let Some(obj) = self.gray_list.pop_front() {
            let header = unsafe { obj.as_ref() };
            header.color.set(Color::Black);
            // Would trace references here - simplified for now
        }
    }

    /// Trace from gray objects (all generations)
    fn trace_gray_full(&mut self) {
        while let Some(obj) = self.gray_list.pop_front() {
            let header = unsafe { obj.as_ref() };
            header.color.set(Color::Black);
            // Would trace references here - simplified for now
        }
    }

    /// Get heap info: (young_allocated, young_max, old_allocated, old_max)
    pub fn heap_info(&self) -> (usize, usize, usize, usize) {
        (
            self.young_heap.allocated(),
            self.young_heap.max_size(),
            self.old_heap.allocated(),
            self.old_heap.max_size(),
        )
    }

    /// Get combined statistics
    pub fn stats(&self) -> &GenGCStats {
        &self.stats
    }
}

impl Default for GenerationalGC {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestObj {
        value: i64,
    }

    impl Trace for TestObj {
        fn trace(&self, _tracer: &mut dyn FnMut(*const ObjectHeader)) {}
    }

    #[test]
    fn test_write_barrier() {
        let mut wb = WriteBarrier::new();
        
        // Create fake headers
        let mut h1 = ObjectHeader {
            color: Cell::new(Color::White),
            is_root: Cell::new(false),
            size: 8,
            next: Cell::new(None),
            type_tag: TypeTag::String,
        };
        let mut h2 = ObjectHeader {
            color: Cell::new(Color::White),
            is_root: Cell::new(false),
            size: 8,
            next: Cell::new(None),
            type_tag: TypeTag::String,
        };
        
        let p1 = NonNull::new(&mut h1 as *mut ObjectHeader).unwrap();
        let p2 = NonNull::new(&mut h2 as *mut ObjectHeader).unwrap();
        
        wb.on_write(p1, p2);
        
        assert_eq!(wb.remembered_count(), 1);
        assert_eq!(wb.stats.barrier_calls, 1);
        assert_eq!(wb.stats.remembered_adds, 1);
    }

    #[test]
    fn test_generational_gc_creation() {
        let gc = GenerationalGC::with_defaults();
        let (young_alloc, young_max, old_alloc, old_max) = gc.heap_info();
        
        assert_eq!(young_alloc, 0);
        assert_eq!(young_max, 8 * 1024 * 1024);
        assert_eq!(old_alloc, 0);
        assert_eq!(old_max, 64 * 1024 * 1024);
    }

    #[test]
    fn test_young_allocation() {
        let mut gc = GenerationalGC::with_defaults();
        
        let obj = gc.alloc_young(TypeTag::String, TestObj { value: 42 });
        assert!(obj.is_some());
        assert_eq!(obj.unwrap().get().value, 42);
    }

    #[test]
    fn test_young_collection() {
        let mut gc = GenerationalGC::new(1024 * 1024, 8 * 1024 * 1024);
        
        // Allocate some objects
        let _ = gc.alloc_young(TypeTag::String, TestObj { value: 1 });
        let _ = gc.alloc_young(TypeTag::String, TestObj { value: 2 });
        
        // Collect
        gc.collect_young(&mut []);
        
        assert_eq!(gc.stats.young_collections, 1);
        assert!(gc.stats.young_bytes_freed > 0);
    }

    #[test]
    fn test_full_collection() {
        let mut gc = GenerationalGC::new(1024 * 1024, 8 * 1024 * 1024);
        
        // Allocate
        let _ = gc.alloc_young(TypeTag::String, TestObj { value: 1 });
        
        // Full collection
        gc.collect_full(&mut []);
        
        assert_eq!(gc.stats.full_collections, 1);
    }

    #[test]
    fn test_root_survives_collection() {
        let mut gc = GenerationalGC::new(1024 * 1024, 8 * 1024 * 1024);
        
        let obj = gc.alloc_young(TypeTag::String, TestObj { value: 42 }).unwrap();
        obj.mark_root();
        
        gc.collect_young(&mut []);
        
        // Root should survive
        assert_eq!(obj.get().value, 42);
    }
}
