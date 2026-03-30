//! Garbage collector integration tests

use vector::gc::{GC, TypeTag, Trace, ObjectHeader};

struct TestObject {
    value: i64,
    children: Vec<*const ObjectHeader>,
}

impl Trace for TestObject {
    fn trace(&self, tracer: &mut dyn FnMut(*const ObjectHeader)) {
        for &child in &self.children {
            tracer(child);
        }
    }
}

#[test]
fn test_gc_creation() {
    let gc = GC::new();
    assert_eq!(gc.stats().collections, 0);
    assert_eq!(gc.stats().bytes_freed, 0);
}

#[test]
fn test_gc_with_custom_heap() {
    let gc = GC::with_heap_size(1024 * 1024);
    let (_, max_size, _) = gc.heap_info();
    assert_eq!(max_size, 1024 * 1024);
}

#[test]
fn test_gc_allocation() {
    let mut gc = GC::new();
    let obj = gc.alloc(TypeTag::String, TestObject { value: 42, children: vec![] });
    assert!(obj.is_some());
    assert_eq!(obj.unwrap().get().value, 42);
}

#[test]
fn test_gc_collect_unreachable() {
    let mut gc = GC::with_heap_size(1024 * 1024);
    gc.set_auto_collect(false);
    
    // Allocate objects without keeping references
    for i in 0..10 {
        let _ = gc.alloc(TypeTag::String, TestObject { value: i, children: vec![] });
    }
    
    let objects_before = gc.heap.iter_objects().count();
    assert_eq!(objects_before, 10);
    
    // Collect with no roots - all should be freed
    gc.collect_with_roots(&mut []);
    
    assert_eq!(gc.stats().collections, 1);
    assert!(gc.stats().last_freed_objects > 0);
}

#[test]
fn test_gc_preserve_roots() {
    let mut gc = GC::with_heap_size(1024 * 1024);
    gc.set_auto_collect(false);
    
    // Allocate and mark as root
    let root = gc.alloc(TypeTag::String, TestObject { value: 42, children: vec![] }).unwrap();
    root.mark_root();
    
    // Allocate unreachable objects
    for i in 0..5 {
        let _ = gc.alloc(TypeTag::String, TestObject { value: i, children: vec![] });
    }
    
    gc.collect_with_roots(&mut []);
    
    // Root should survive, others should be freed
    assert_eq!(gc.heap.iter_objects().count(), 1);
    assert_eq!(root.get().value, 42);
}

#[test]
fn test_gc_explicit_roots() {
    let mut gc = GC::with_heap_size(1024 * 1024);
    gc.set_auto_collect(false);
    
    let obj1 = gc.alloc(TypeTag::String, TestObject { value: 1, children: vec![] }).unwrap();
    let _ = gc.alloc(TypeTag::String, TestObject { value: 2, children: vec![] }); // unreachable
    let obj3 = gc.alloc(TypeTag::String, TestObject { value: 3, children: vec![] }).unwrap();
    
    // Only obj1 and obj3 are roots
    let roots = vec![obj1.header_ptr(), obj3.header_ptr()];
    gc.collect_with_roots(&mut roots.into_iter().collect::<Vec<_>>());
    
    assert_eq!(gc.stats().last_freed_objects, 1);
    assert_eq!(gc.heap.iter_objects().count(), 2);
}

#[test]
fn test_gc_root_callback() {
    let mut gc = GC::with_heap_size(1024 * 1024);
    gc.set_auto_collect(false);
    
    let obj = gc.alloc(TypeTag::String, TestObject { value: 42, children: vec![] }).unwrap();
    let obj_ptr = obj.header_ptr();
    
    // Register callback that returns our object as a root
    gc.add_root_callback(Box::new(move |visit| {
        visit(obj_ptr);
    }));
    
    // Create unreachable object
    let _ = gc.alloc(TypeTag::String, TestObject { value: 99, children: vec![] });
    
    gc.collect();
    
    // Root should survive
    assert_eq!(gc.stats().last_freed_objects, 1);
}

#[test]
fn test_gc_multiple_collections() {
    let mut gc = GC::with_heap_size(1024 * 1024);
    gc.set_auto_collect(false);
    
    for _ in 0..5 {
        // Allocate some objects
        for i in 0..10 {
            let _ = gc.alloc(TypeTag::String, TestObject { value: i, children: vec![] });
        }
        
        // Collect them
        gc.collect_with_roots(&mut []);
    }
    
    assert_eq!(gc.stats().collections, 5);
}

#[test]
fn test_gc_stats_tracking() {
    let mut gc = GC::with_heap_size(1024 * 1024);
    gc.set_auto_collect(false);
    
    // Initial stats
    assert_eq!(gc.stats().collections, 0);
    assert_eq!(gc.stats().bytes_freed, 0);
    
    // Allocate and collect
    for _ in 0..5 {
        let _ = gc.alloc(TypeTag::String, TestObject { value: 1, children: vec![] });
    }
    gc.collect_with_roots(&mut []);
    
    // Stats should be updated
    assert_eq!(gc.stats().collections, 1);
    assert!(gc.stats().bytes_freed > 0);
    assert!(gc.stats().last_collect_us >= 0);
}

#[test]
fn test_gc_heap_info() {
    let mut gc = GC::with_heap_size(2 * 1024 * 1024);
    
    let (_, max_size, _) = gc.heap_info();
    assert_eq!(max_size, 2 * 1024 * 1024);
}

#[test]
fn test_gc_clear_root_callbacks() {
    let mut gc = GC::new();
    
    gc.add_root_callback(Box::new(|_| {}));
    gc.add_root_callback(Box::new(|_| {}));
    
    gc.clear_root_callbacks();
    
    // Should work without error
    gc.collect();
}

#[test]
fn test_gc_toggle_auto_collect() {
    let mut gc = GC::new();
    
    gc.set_auto_collect(false);
    // Allocation shouldn't trigger collection
    
    gc.set_auto_collect(true);
    // Allocation may trigger collection if threshold exceeded
}

#[test]
fn test_gc_colors() {
    let mut gc = GC::with_heap_size(1024 * 1024);
    gc.set_auto_collect(false);
    
    let obj = gc.alloc(TypeTag::String, TestObject { value: 1, children: vec![] }).unwrap();
    
    // Mark as root and collect
    obj.mark_root();
    gc.collect_with_roots(&mut []);
    
    // Object should still be accessible after collection
    assert_eq!(obj.get().value, 1);
}
