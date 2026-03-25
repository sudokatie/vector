//! Heap allocation for GC-managed objects

use std::alloc::{alloc, dealloc, Layout};
use std::cell::{Cell, UnsafeCell};
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::ops::{Deref, DerefMut};

/// GC color for tri-color marking
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    /// Not yet seen
    White = 0,
    /// Seen but children not yet scanned
    Gray = 1,
    /// Fully scanned
    Black = 2,
}

/// Object header stored before each allocated object
#[repr(C)]
pub struct ObjectHeader {
    /// GC color for marking
    pub color: Cell<Color>,
    /// Whether this object is being traced
    pub is_root: Cell<bool>,
    /// Size of the object (excluding header)
    pub size: usize,
    /// Next object in the heap allocation list
    pub next: Cell<Option<NonNull<ObjectHeader>>>,
    /// Object type tag (for debugging/dispatch)
    pub type_tag: TypeTag,
}

/// Type tags for heap objects
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TypeTag {
    String = 0,
    Array = 1,
    Table = 2,
    Closure = 3,
    Upvalue = 4,
}

/// Trait for GC-traced objects
pub trait Trace {
    /// Trace all references in this object
    fn trace(&self, tracer: &mut dyn FnMut(*const ObjectHeader));
}

/// A managed heap for GC objects
pub struct Heap {
    /// Maximum heap size
    max_size: usize,
    /// Currently allocated bytes
    allocated: usize,
    /// Head of the allocation list
    head: Cell<Option<NonNull<ObjectHeader>>>,
    /// Threshold for triggering collection
    threshold: usize,
    /// Growth factor for threshold
    growth_factor: f64,
}

impl Heap {
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size,
            allocated: 0,
            head: Cell::new(None),
            threshold: max_size / 8, // Initial threshold at 12.5%
            growth_factor: 1.5,
        }
    }

    pub fn allocated(&self) -> usize {
        self.allocated
    }

    pub fn max_size(&self) -> usize {
        self.max_size
    }

    pub fn threshold(&self) -> usize {
        self.threshold
    }

    /// Check if we should trigger a collection
    pub fn should_collect(&self) -> bool {
        self.allocated >= self.threshold
    }

    /// Allocate an object on the heap
    /// 
    /// # Safety
    /// The returned pointer must be properly initialized before use
    pub fn allocate<T: Trace>(&mut self, type_tag: TypeTag) -> Option<NonNull<GcBox<T>>> {
        let layout = Self::layout_for::<T>();
        let total_size = layout.size();

        if self.allocated + total_size > self.max_size {
            return None;
        }

        // Allocate memory
        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            return None;
        }

        let gc_box = ptr as *mut GcBox<T>;
        
        // Initialize header
        unsafe {
            (*gc_box).header = ObjectHeader {
                color: Cell::new(Color::White),
                is_root: Cell::new(false),
                size: std::mem::size_of::<T>(),
                next: Cell::new(self.head.get()),
                type_tag,
            };
        }

        // Add to allocation list
        let header_ptr = unsafe { NonNull::new_unchecked(&mut (*gc_box).header) };
        self.head.set(Some(header_ptr));
        self.allocated += total_size;

        NonNull::new(gc_box)
    }

    /// Allocate and initialize an object
    pub fn alloc<T: Trace>(&mut self, type_tag: TypeTag, value: T) -> Option<GcRef<T>> {
        let ptr = self.allocate::<T>(type_tag)?;
        unsafe {
            std::ptr::write(&mut (*ptr.as_ptr()).data, UnsafeCell::new(value));
        }
        Some(GcRef { ptr, _marker: PhantomData })
    }

    /// Get layout for GcBox<T>
    fn layout_for<T>() -> Layout {
        Layout::new::<GcBox<T>>()
    }

    /// Iterate over all objects (for GC)
    pub fn iter_objects(&self) -> ObjectIter {
        ObjectIter { current: self.head.get() }
    }

    /// Sweep unmarked objects
    pub fn sweep(&mut self) -> usize {
        let mut freed = 0;
        let mut prev: Option<NonNull<ObjectHeader>> = None;
        let mut current = self.head.get();

        while let Some(obj_ptr) = current {
            let header = unsafe { obj_ptr.as_ref() };
            let next = header.next.get();

            if header.color.get() == Color::White && !header.is_root.get() {
                // Remove from list
                if let Some(p) = prev {
                    unsafe { p.as_ref() }.next.set(next);
                } else {
                    self.head.set(next);
                }

                // Free memory
                let layout = Layout::new::<ObjectHeader>().extend(
                    Layout::from_size_align(header.size, 8).unwrap()
                ).unwrap().0;
                
                unsafe {
                    dealloc(obj_ptr.as_ptr() as *mut u8, layout);
                }
                
                freed += layout.size();
            } else {
                // Reset color for next cycle
                header.color.set(Color::White);
                prev = Some(obj_ptr);
            }

            current = next;
        }

        self.allocated -= freed;
        freed
    }

    /// Update threshold after collection
    pub fn update_threshold(&mut self) {
        self.threshold = ((self.allocated as f64) * self.growth_factor) as usize;
        self.threshold = self.threshold.max(self.max_size / 16).min(self.max_size);
    }
}

impl Default for Heap {
    fn default() -> Self {
        Self::new(64 * 1024 * 1024) // 64MB default
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        // Free all remaining objects
        let mut current = self.head.get();
        while let Some(obj_ptr) = current {
            let header = unsafe { obj_ptr.as_ref() };
            current = header.next.get();
            
            let layout = Layout::new::<ObjectHeader>().extend(
                Layout::from_size_align(header.size, 8).unwrap()
            ).unwrap().0;
            
            unsafe {
                dealloc(obj_ptr.as_ptr() as *mut u8, layout);
            }
        }
    }
}

/// Iterator over heap objects
pub struct ObjectIter {
    current: Option<NonNull<ObjectHeader>>,
}

impl Iterator for ObjectIter {
    type Item = NonNull<ObjectHeader>;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current?;
        self.current = unsafe { current.as_ref() }.next.get();
        Some(current)
    }
}

/// A box on the GC heap containing header and data
#[repr(C)]
pub struct GcBox<T> {
    pub header: ObjectHeader,
    pub data: UnsafeCell<T>,
}

/// A reference to a GC-managed object
pub struct GcRef<T> {
    ptr: NonNull<GcBox<T>>,
    _marker: PhantomData<T>,
}

impl<T> GcRef<T> {
    /// Get the header for this object
    pub fn header(&self) -> &ObjectHeader {
        unsafe { &(*self.ptr.as_ptr()).header }
    }

    /// Mark this reference as a root (prevents collection)
    pub fn mark_root(&self) {
        self.header().is_root.set(true);
    }

    /// Unmark this reference as a root
    pub fn unmark_root(&self) {
        self.header().is_root.set(false);
    }

    /// Get raw pointer to the header
    pub fn header_ptr(&self) -> *const ObjectHeader {
        &unsafe { &*self.ptr.as_ptr() }.header
    }

    /// Get mutable access to the data
    /// 
    /// # Safety
    /// Caller must ensure no other references exist
    pub unsafe fn get_mut(&self) -> &mut T {
        &mut *(*self.ptr.as_ptr()).data.get()
    }

    /// Get immutable access to the data
    pub fn get(&self) -> &T {
        unsafe { &*(*self.ptr.as_ptr()).data.get() }
    }

    /// Borrow the inner value (like RefCell::borrow)
    pub fn borrow(&self) -> GcBorrow<'_, T> {
        GcBorrow { inner: self.get() }
    }

    /// Borrow the inner value mutably (like RefCell::borrow_mut)
    /// 
    /// # Safety
    /// Caller must ensure exclusive access
    pub fn borrow_mut(&self) -> GcBorrowMut<'_, T> {
        GcBorrowMut { inner: unsafe { self.get_mut() } }
    }
}

impl<T> Clone for GcRef<T> {
    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr,
            _marker: PhantomData,
        }
    }
}

impl<T> Copy for GcRef<T> {}

impl<T: PartialEq> PartialEq for GcRef<T> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.ptr.as_ptr(), other.ptr.as_ptr())
    }
}

impl<T: Eq> Eq for GcRef<T> {}

impl<T: std::hash::Hash> std::hash::Hash for GcRef<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ptr.as_ptr().hash(state);
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for GcRef<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GcRef({:?})", self.get())
    }
}

/// Immutable borrow of a GC object
pub struct GcBorrow<'a, T> {
    inner: &'a T,
}

impl<T> Deref for GcBorrow<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.inner
    }
}

/// Mutable borrow of a GC object
pub struct GcBorrowMut<'a, T> {
    inner: &'a mut T,
}

impl<T> Deref for GcBorrowMut<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.inner
    }
}

impl<T> DerefMut for GcBorrowMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestObject {
        value: i64,
    }

    impl Trace for TestObject {
        fn trace(&self, _tracer: &mut dyn FnMut(*const ObjectHeader)) {
            // No references to trace
        }
    }

    #[test]
    fn test_heap_allocation() {
        let mut heap = Heap::new(1024 * 1024);
        let obj = heap.alloc(TypeTag::String, TestObject { value: 42 });
        assert!(obj.is_some());
        let obj = obj.unwrap();
        assert_eq!(obj.get().value, 42);
    }

    #[test]
    fn test_heap_limits() {
        let mut heap = Heap::new(64); // Very small heap
        let obj1 = heap.alloc(TypeTag::String, TestObject { value: 1 });
        assert!(obj1.is_some());
        
        // Second allocation might fail due to size
        let obj2 = heap.alloc(TypeTag::String, TestObject { value: 2 });
        // May or may not succeed depending on overhead
        let _ = obj2;
    }

    #[test]
    fn test_gc_colors() {
        let mut heap = Heap::new(1024 * 1024);
        let obj = heap.alloc(TypeTag::String, TestObject { value: 42 }).unwrap();
        
        assert_eq!(obj.header().color.get(), Color::White);
        obj.header().color.set(Color::Gray);
        assert_eq!(obj.header().color.get(), Color::Gray);
        obj.header().color.set(Color::Black);
        assert_eq!(obj.header().color.get(), Color::Black);
    }

    #[test]
    fn test_object_iteration() {
        let mut heap = Heap::new(1024 * 1024);
        heap.alloc(TypeTag::String, TestObject { value: 1 });
        heap.alloc(TypeTag::Array, TestObject { value: 2 });
        heap.alloc(TypeTag::Table, TestObject { value: 3 });
        
        let count = heap.iter_objects().count();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_sweep_unreachable() {
        let mut heap = Heap::new(1024 * 1024);
        heap.alloc(TypeTag::String, TestObject { value: 1 });
        heap.alloc(TypeTag::String, TestObject { value: 2 });
        
        let allocated_before = heap.allocated();
        let freed = heap.sweep();
        
        // Both objects were white (unmarked), should be freed
        assert!(freed > 0);
        assert!(heap.allocated() < allocated_before);
    }

    #[test]
    fn test_root_survives_sweep() {
        let mut heap = Heap::new(1024 * 1024);
        let obj = heap.alloc(TypeTag::String, TestObject { value: 42 }).unwrap();
        obj.mark_root();
        
        let objects_before = heap.iter_objects().count();
        heap.sweep();
        let objects_after = heap.iter_objects().count();
        
        // Root should survive
        assert_eq!(objects_before, objects_after);
    }

    #[test]
    fn test_gc_borrow() {
        let mut heap = Heap::new(1024 * 1024);
        let obj = heap.alloc(TypeTag::String, TestObject { value: 42 }).unwrap();
        
        {
            let borrow = obj.borrow();
            assert_eq!(borrow.value, 42);
        }
        
        {
            let mut borrow = obj.borrow_mut();
            borrow.value = 100;
        }
        
        assert_eq!(obj.get().value, 100);
    }
}
