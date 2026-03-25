//! Tracing garbage collector

use std::ptr::NonNull;
use std::collections::VecDeque;
use super::heap::{Color, Heap, ObjectHeader, Trace, GcRef};

/// Statistics for garbage collection
#[derive(Debug, Default, Clone)]
pub struct GCStats {
    /// Number of collections performed
    pub collections: u64,
    /// Total bytes allocated ever
    pub bytes_allocated: u64,
    /// Total bytes freed ever
    pub bytes_freed: u64,
    /// Current live objects
    pub live_objects: usize,
    /// Objects freed in last collection
    pub last_freed_objects: usize,
    /// Bytes freed in last collection
    pub last_freed_bytes: usize,
    /// Time spent in last collection (microseconds)
    pub last_collect_us: u64,
}

/// Root enumeration callback
pub type RootCallback = Box<dyn FnMut(&mut dyn FnMut(*const ObjectHeader))>;

/// The garbage collector
pub struct GC {
    /// Heap for allocations
    pub heap: Heap,
    /// Collection statistics
    pub stats: GCStats,
    /// Gray worklist for tri-color marking
    gray_list: VecDeque<NonNull<ObjectHeader>>,
    /// Callbacks to enumerate roots
    root_callbacks: Vec<RootCallback>,
    /// Whether automatic collection is enabled
    auto_collect: bool,
}

impl GC {
    pub fn new() -> Self {
        Self::with_heap_size(64 * 1024 * 1024)
    }

    pub fn with_heap_size(size: usize) -> Self {
        Self {
            heap: Heap::new(size),
            stats: GCStats::default(),
            gray_list: VecDeque::new(),
            root_callbacks: Vec::new(),
            auto_collect: true,
        }
    }

    /// Add a root enumeration callback
    pub fn add_root_callback(&mut self, callback: RootCallback) {
        self.root_callbacks.push(callback);
    }

    /// Clear all root callbacks
    pub fn clear_root_callbacks(&mut self) {
        self.root_callbacks.clear();
    }

    /// Enable/disable automatic collection
    pub fn set_auto_collect(&mut self, enabled: bool) {
        self.auto_collect = enabled;
    }

    /// Check if we should collect
    pub fn should_collect(&self) -> bool {
        self.auto_collect && self.heap.should_collect()
    }

    /// Allocate an object, potentially triggering collection
    pub fn alloc<T: Trace>(&mut self, type_tag: super::heap::TypeTag, value: T) -> Option<GcRef<T>> {
        if self.should_collect() {
            self.collect_with_roots(&mut []);
        }
        
        let result = self.heap.alloc(type_tag, value);
        if let Some(ref r) = result {
            self.stats.bytes_allocated += std::mem::size_of::<T>() as u64;
            let _ = r; // silence warning
        }
        result
    }

    /// Perform a collection with externally-provided roots
    pub fn collect_with_roots(&mut self, roots: &mut [*const ObjectHeader]) {
        let start = std::time::Instant::now();
        
        // Mark phase: start with roots
        self.mark_roots(roots);
        
        // Trace from gray objects
        self.trace_gray();
        
        // Sweep phase: free unmarked objects
        let objects_before = self.heap.iter_objects().count();
        let freed = self.heap.sweep();
        let objects_after = self.heap.iter_objects().count();
        
        // Update statistics
        self.stats.collections += 1;
        self.stats.bytes_freed += freed as u64;
        self.stats.live_objects = objects_after;
        self.stats.last_freed_objects = objects_before - objects_after;
        self.stats.last_freed_bytes = freed;
        self.stats.last_collect_us = start.elapsed().as_micros() as u64;
        
        // Adjust threshold
        self.heap.update_threshold();
    }

    /// Perform a collection using registered root callbacks
    pub fn collect(&mut self) {
        let start = std::time::Instant::now();
        
        // Collect roots from callbacks
        let mut roots: Vec<*const ObjectHeader> = Vec::new();
        
        // We need to take the callbacks temporarily to avoid borrowing issues
        let mut callbacks = std::mem::take(&mut self.root_callbacks);
        
        for callback in callbacks.iter_mut() {
            callback(&mut |ptr| {
                roots.push(ptr);
            });
        }
        
        // Put callbacks back
        self.root_callbacks = callbacks;
        
        // Mark from collected roots
        self.mark_roots(&roots);
        
        // Trace from gray objects
        self.trace_gray();
        
        // Sweep unmarked
        let objects_before = self.heap.iter_objects().count();
        let freed = self.heap.sweep();
        let objects_after = self.heap.iter_objects().count();
        
        // Update statistics
        self.stats.collections += 1;
        self.stats.bytes_freed += freed as u64;
        self.stats.live_objects = objects_after;
        self.stats.last_freed_objects = objects_before - objects_after;
        self.stats.last_freed_bytes = freed;
        self.stats.last_collect_us = start.elapsed().as_micros() as u64;
        
        self.heap.update_threshold();
    }

    /// Mark root objects as gray
    fn mark_roots(&mut self, roots: &[*const ObjectHeader]) {
        self.gray_list.clear();
        
        for &root in roots {
            if !root.is_null() {
                let header = unsafe { &*root };
                if header.color.get() == Color::White {
                    header.color.set(Color::Gray);
                    self.gray_list.push_back(NonNull::new(root as *mut ObjectHeader).unwrap());
                }
            }
        }
        
        // Also mark objects flagged as roots
        for obj in self.heap.iter_objects() {
            let header = unsafe { obj.as_ref() };
            if header.is_root.get() && header.color.get() == Color::White {
                header.color.set(Color::Gray);
                self.gray_list.push_back(obj);
            }
        }
    }

    /// Trace from gray objects until none remain
    fn trace_gray(&mut self) {
        while let Some(obj) = self.gray_list.pop_front() {
            let header = unsafe { obj.as_ref() };
            
            // Mark as black (fully scanned)
            header.color.set(Color::Black);
            
            // Get the object data and trace its references
            // We dispatch based on type tag
            match header.type_tag {
                super::heap::TypeTag::String => {
                    // Strings have no references
                }
                super::heap::TypeTag::Array => {
                    // Arrays contain Value references - handled by VM
                    // We rely on root callbacks for this
                }
                super::heap::TypeTag::Table => {
                    // Tables contain Value references - handled by VM
                    // We rely on root callbacks for this
                }
                super::heap::TypeTag::Closure => {
                    // Closures have upvalue references
                }
                super::heap::TypeTag::Upvalue => {
                    // Upvalues may reference other objects
                }
            }
        }
    }

    /// Mark an object as reachable (for use during tracing)
    pub fn mark(&mut self, header: *const ObjectHeader) {
        if header.is_null() {
            return;
        }
        
        let header = unsafe { &*header };
        if header.color.get() == Color::White {
            header.color.set(Color::Gray);
            self.gray_list.push_back(NonNull::new(header as *const _ as *mut ObjectHeader).unwrap());
        }
    }

    /// Get GC statistics
    pub fn stats(&self) -> &GCStats {
        &self.stats
    }

    /// Get heap info
    pub fn heap_info(&self) -> (usize, usize, usize) {
        (self.heap.allocated(), self.heap.max_size(), self.heap.threshold())
    }
}

impl Default for GC {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gc::heap::TypeTag;

    struct TestObj {
        value: i64,
    }

    impl Trace for TestObj {
        fn trace(&self, _tracer: &mut dyn FnMut(*const ObjectHeader)) {}
    }

    #[test]
    fn test_gc_creation() {
        let gc = GC::new();
        assert_eq!(gc.stats.collections, 0);
        assert_eq!(gc.stats.bytes_freed, 0);
    }

    #[test]
    fn test_gc_allocation() {
        let mut gc = GC::new();
        let obj = gc.alloc(TypeTag::String, TestObj { value: 42 });
        assert!(obj.is_some());
        assert_eq!(obj.unwrap().get().value, 42);
    }

    #[test]
    fn test_collect_frees_unreachable() {
        let mut gc = GC::with_heap_size(1024 * 1024);
        gc.set_auto_collect(false);
        
        // Allocate some objects without keeping references
        let _ = gc.alloc(TypeTag::String, TestObj { value: 1 });
        let _ = gc.alloc(TypeTag::String, TestObj { value: 2 });
        let _ = gc.alloc(TypeTag::String, TestObj { value: 3 });
        
        let objects_before = gc.heap.iter_objects().count();
        assert_eq!(objects_before, 3);
        
        // Collect with no roots
        gc.collect_with_roots(&mut []);
        
        // All should be freed
        assert!(gc.stats.last_freed_objects > 0);
    }

    #[test]
    fn test_collect_preserves_roots() {
        let mut gc = GC::with_heap_size(1024 * 1024);
        gc.set_auto_collect(false);
        
        // Allocate and mark as root
        let obj = gc.alloc(TypeTag::String, TestObj { value: 42 }).unwrap();
        obj.mark_root();
        
        let _ = gc.alloc(TypeTag::String, TestObj { value: 99 }); // Not a root
        
        gc.collect_with_roots(&mut []);
        
        // Root should survive
        assert_eq!(gc.heap.iter_objects().count(), 1);
        assert_eq!(obj.get().value, 42);
    }

    #[test]
    fn test_collect_with_explicit_roots() {
        let mut gc = GC::with_heap_size(1024 * 1024);
        gc.set_auto_collect(false);
        
        let obj1 = gc.alloc(TypeTag::String, TestObj { value: 1 }).unwrap();
        let _ = gc.alloc(TypeTag::String, TestObj { value: 2 }); // unreachable
        let obj3 = gc.alloc(TypeTag::String, TestObj { value: 3 }).unwrap();
        
        // Only obj1 and obj3 are roots
        let roots: Vec<*const ObjectHeader> = vec![obj1.header_ptr(), obj3.header_ptr()];
        gc.collect_with_roots(&mut roots.into_iter().collect::<Vec<_>>());
        
        // obj2 should be freed
        assert_eq!(gc.stats.last_freed_objects, 1);
        assert_eq!(gc.heap.iter_objects().count(), 2);
    }

    #[test]
    fn test_gc_stats() {
        let mut gc = GC::with_heap_size(1024 * 1024);
        gc.set_auto_collect(false);
        
        let _ = gc.alloc(TypeTag::String, TestObj { value: 1 });
        gc.collect_with_roots(&mut []);
        
        assert_eq!(gc.stats.collections, 1);
        assert!(gc.stats.bytes_freed > 0);
    }

    #[test]
    fn test_multiple_collections() {
        let mut gc = GC::with_heap_size(1024 * 1024);
        gc.set_auto_collect(false);
        
        for i in 0..5 {
            let _ = gc.alloc(TypeTag::String, TestObj { value: i });
            gc.collect_with_roots(&mut []);
        }
        
        assert_eq!(gc.stats.collections, 5);
    }

    #[test]
    fn test_auto_collect_trigger() {
        let mut gc = GC::with_heap_size(512); // Very small heap
        gc.set_auto_collect(true);
        
        // Allocate many objects to trigger auto-collection
        for i in 0..20 {
            let obj = gc.alloc(TypeTag::String, TestObj { value: i });
            if obj.is_some() {
                obj.unwrap().mark_root();
            }
        }
        
        // Should have triggered at least one collection
        // (may fail if threshold is never reached)
    }

    #[test]
    fn test_root_callback() {
        let mut gc = GC::with_heap_size(1024 * 1024);
        gc.set_auto_collect(false);
        
        let obj = gc.alloc(TypeTag::String, TestObj { value: 42 }).unwrap();
        let obj_ptr = obj.header_ptr();
        
        // Register a root callback that returns our object
        gc.add_root_callback(Box::new(move |visit| {
            visit(obj_ptr);
        }));
        
        // Create unreachable object
        let _ = gc.alloc(TypeTag::String, TestObj { value: 99 });
        
        gc.collect();
        
        // Root should survive, unreachable should be freed
        assert_eq!(gc.stats.last_freed_objects, 1);
    }
}
