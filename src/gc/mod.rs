//! Garbage collector
//!
//! This module provides a tracing garbage collector for Vector.
//!
//! # Architecture
//!
//! The GC uses tri-color marking:
//! - White: Not yet seen (candidates for collection)
//! - Gray: Seen but children not yet scanned
//! - Black: Fully scanned (will not be collected)
//!
//! Collection phases:
//! 1. Mark roots as gray
//! 2. Process gray objects, marking children gray and object black
//! 3. Sweep all remaining white objects
//!
//! # Generational Collection
//!
//! The generational GC divides the heap into:
//! - Young generation (nursery): Small, frequently collected
//! - Old generation: Larger, collected less often
//!
//! Objects that survive multiple young collections are promoted to old.
//! A write barrier tracks cross-generational references.
//!
//! # Usage
//!
//! ```ignore
//! use vector::gc::{GC, TypeTag};
//!
//! let mut gc = GC::new();
//!
//! // Allocate an object
//! let obj = gc.alloc(TypeTag::String, MyString::new("hello"));
//!
//! // Mark as root to prevent collection
//! obj.mark_root();
//!
//! // Trigger collection
//! gc.collect();
//! ```

pub mod heap;
pub mod trace;
pub mod generational;

pub use heap::{
    Color, GcBox, GcBorrow, GcBorrowMut, GcRef, Heap, 
    ObjectHeader, Trace, TypeTag,
};
pub use trace::{GCStats, GC};
pub use generational::{GenerationalGC, GenGCStats, WriteBarrier, WriteBarrierStats, Generation};
