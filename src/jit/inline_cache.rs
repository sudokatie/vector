//! Inline caching for property access optimization

use fnv::FnvHashMap;
use std::hash::Hash;

/// Shape ID - identifies a particular object layout
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShapeId(pub u32);

impl ShapeId {
    pub const UNKNOWN: ShapeId = ShapeId(0);
    
    pub fn is_unknown(&self) -> bool {
        *self == Self::UNKNOWN
    }
}

/// Property slot in an object
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropertySlot {
    /// Offset within the object's property array
    pub offset: u16,
    /// Whether this is an own property (vs inherited)
    pub is_own: bool,
}

/// A single inline cache entry
#[derive(Debug, Clone)]
pub struct ICEntry {
    /// Expected shape of the object
    pub shape_id: ShapeId,
    /// Cached property slot
    pub slot: PropertySlot,
    /// Number of hits
    pub hits: u32,
    /// Number of misses
    pub misses: u32,
}

impl ICEntry {
    pub fn new(shape_id: ShapeId, slot: PropertySlot) -> Self {
        Self {
            shape_id,
            slot,
            hits: 0,
            misses: 0,
        }
    }

    /// Record a cache hit
    pub fn hit(&mut self) {
        self.hits = self.hits.saturating_add(1);
    }

    /// Record a cache miss
    pub fn miss(&mut self) {
        self.misses = self.misses.saturating_add(1);
    }

    /// Get hit rate
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    /// Check if this entry is megamorphic (too many misses)
    pub fn is_megamorphic(&self) -> bool {
        self.misses > 100 && self.hit_rate() < 0.5
    }
}

/// Inline cache state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ICState {
    /// No cached information yet
    Uninitialized,
    /// Single shape seen (monomorphic)
    Monomorphic,
    /// Few shapes seen (polymorphic)
    Polymorphic,
    /// Too many shapes (megamorphic) - give up caching
    Megamorphic,
}

/// Polymorphic inline cache - handles multiple shapes
#[derive(Debug, Clone)]
pub struct PolymorphicIC {
    /// Cache entries (up to MAX_ENTRIES)
    entries: Vec<ICEntry>,
    /// Current state
    state: ICState,
    /// Maximum entries before going megamorphic
    max_entries: usize,
}

impl PolymorphicIC {
    const DEFAULT_MAX_ENTRIES: usize = 4;

    pub fn new() -> Self {
        Self {
            entries: Vec::with_capacity(Self::DEFAULT_MAX_ENTRIES),
            state: ICState::Uninitialized,
            max_entries: Self::DEFAULT_MAX_ENTRIES,
        }
    }

    /// Look up a property, returning the slot if cached
    pub fn lookup(&mut self, shape_id: ShapeId) -> Option<PropertySlot> {
        for entry in &mut self.entries {
            if entry.shape_id == shape_id {
                entry.hit();
                return Some(entry.slot);
            }
        }
        
        // Miss on all entries
        for entry in &mut self.entries {
            entry.miss();
        }
        
        None
    }

    /// Add a new cache entry
    pub fn add(&mut self, shape_id: ShapeId, slot: PropertySlot) {
        // Check if already present
        if self.entries.iter().any(|e| e.shape_id == shape_id) {
            return;
        }

        if self.entries.len() >= self.max_entries {
            // Check if any entry is megamorphic
            if self.entries.iter().any(|e| e.is_megamorphic()) {
                self.state = ICState::Megamorphic;
                return;
            }
            
            // Remove worst performing entry
            if let Some(worst_idx) = self.entries
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    a.hit_rate().partial_cmp(&b.hit_rate()).unwrap()
                })
                .map(|(i, _)| i)
            {
                self.entries.remove(worst_idx);
            }
        }

        self.entries.push(ICEntry::new(shape_id, slot));
        self.update_state();
    }

    /// Update the cache state
    fn update_state(&mut self) {
        self.state = match self.entries.len() {
            0 => ICState::Uninitialized,
            1 => ICState::Monomorphic,
            2..=4 => ICState::Polymorphic,
            _ => ICState::Megamorphic,
        };
    }

    /// Get current state
    pub fn state(&self) -> ICState {
        self.state
    }

    /// Check if megamorphic
    pub fn is_megamorphic(&self) -> bool {
        self.state == ICState::Megamorphic
    }

    /// Reset the cache
    pub fn reset(&mut self) {
        self.entries.clear();
        self.state = ICState::Uninitialized;
    }

    /// Get total hit rate
    pub fn hit_rate(&self) -> f64 {
        let (hits, total): (u32, u32) = self.entries.iter()
            .fold((0, 0), |(h, t), e| (h + e.hits, t + e.hits + e.misses));
        
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }
}

impl Default for PolymorphicIC {
    fn default() -> Self {
        Self::new()
    }
}

/// Property access site - tracked for optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AccessSite {
    /// Bytecode offset of the access
    pub offset: u32,
    /// Property name hash
    pub property_hash: u64,
}

impl AccessSite {
    pub fn new(offset: u32, property_name: &str) -> Self {
        use std::hash::Hasher;
        let mut hasher = fnv::FnvHasher::default();
        property_name.hash(&mut hasher);
        Self {
            offset,
            property_hash: hasher.finish(),
        }
    }
}

/// Global inline cache manager
pub struct InlineCacheManager {
    /// Caches keyed by access site
    caches: FnvHashMap<AccessSite, PolymorphicIC>,
    /// Shape ID counter
    next_shape_id: u32,
    /// Statistics
    pub stats: ICStats,
}

#[derive(Debug, Default, Clone)]
pub struct ICStats {
    /// Total lookups
    pub lookups: u64,
    /// Cache hits
    pub hits: u64,
    /// Cache misses
    pub misses: u64,
    /// Number of cache sites
    pub sites: usize,
    /// Number of megamorphic sites
    pub megamorphic_sites: usize,
}

impl InlineCacheManager {
    pub fn new() -> Self {
        Self {
            caches: FnvHashMap::default(),
            next_shape_id: 1, // 0 is reserved for UNKNOWN
            stats: ICStats::default(),
        }
    }

    /// Allocate a new shape ID
    pub fn new_shape_id(&mut self) -> ShapeId {
        let id = ShapeId(self.next_shape_id);
        self.next_shape_id += 1;
        id
    }

    /// Look up a property at an access site
    pub fn lookup(&mut self, site: AccessSite, shape_id: ShapeId) -> Option<PropertySlot> {
        self.stats.lookups += 1;
        
        if let Some(cache) = self.caches.get_mut(&site) {
            if let Some(slot) = cache.lookup(shape_id) {
                self.stats.hits += 1;
                return Some(slot);
            }
        }
        
        self.stats.misses += 1;
        None
    }

    /// Record a property access result
    pub fn record(&mut self, site: AccessSite, shape_id: ShapeId, slot: PropertySlot) {
        let cache = self.caches.entry(site).or_insert_with(|| {
            self.stats.sites += 1;
            PolymorphicIC::new()
        });
        
        let was_megamorphic = cache.is_megamorphic();
        cache.add(shape_id, slot);
        
        if !was_megamorphic && cache.is_megamorphic() {
            self.stats.megamorphic_sites += 1;
        }
    }

    /// Get cache for a site
    pub fn get_cache(&self, site: &AccessSite) -> Option<&PolymorphicIC> {
        self.caches.get(site)
    }

    /// Get hit rate
    pub fn hit_rate(&self) -> f64 {
        if self.stats.lookups == 0 {
            0.0
        } else {
            self.stats.hits as f64 / self.stats.lookups as f64
        }
    }

    /// Clear all caches
    pub fn clear(&mut self) {
        self.caches.clear();
        self.stats.sites = 0;
        self.stats.megamorphic_sites = 0;
    }

    /// Get statistics
    pub fn stats(&self) -> &ICStats {
        &self.stats
    }
}

impl Default for InlineCacheManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Shape descriptor - describes an object's property layout
#[derive(Debug, Clone)]
pub struct Shape {
    /// Shape ID
    pub id: ShapeId,
    /// Property name to slot mapping
    properties: FnvHashMap<String, PropertySlot>,
    /// Parent shape (for prototype chain)
    parent: Option<ShapeId>,
    /// Number of transitions from this shape
    transitions: u32,
}

impl Shape {
    pub fn new(id: ShapeId) -> Self {
        Self {
            id,
            properties: FnvHashMap::default(),
            parent: None,
            transitions: 0,
        }
    }

    /// Add a property
    pub fn add_property(&mut self, name: String) -> PropertySlot {
        let offset = self.properties.len() as u16;
        let slot = PropertySlot { offset, is_own: true };
        self.properties.insert(name, slot);
        slot
    }

    /// Get a property slot
    pub fn get_property(&self, name: &str) -> Option<PropertySlot> {
        self.properties.get(name).copied()
    }

    /// Check if has property
    pub fn has_property(&self, name: &str) -> bool {
        self.properties.contains_key(name)
    }

    /// Get property count
    pub fn property_count(&self) -> usize {
        self.properties.len()
    }
}

/// Shape table - tracks all shapes in the system
pub struct ShapeTable {
    shapes: FnvHashMap<ShapeId, Shape>,
    /// Transition cache: (parent_shape, property_name) -> child_shape
    transitions: FnvHashMap<(ShapeId, String), ShapeId>,
    /// ID allocator
    next_id: u32,
}

impl ShapeTable {
    pub fn new() -> Self {
        let mut table = Self {
            shapes: FnvHashMap::default(),
            transitions: FnvHashMap::default(),
            next_id: 1,
        };
        
        // Create root shape
        let root = Shape::new(ShapeId(0));
        table.shapes.insert(ShapeId(0), root);
        
        table
    }

    /// Get or create a shape with an additional property
    pub fn transition(&mut self, from: ShapeId, property: &str) -> ShapeId {
        let key = (from, property.to_string());
        
        if let Some(&shape_id) = self.transitions.get(&key) {
            return shape_id;
        }

        // Create new shape
        let new_id = ShapeId(self.next_id);
        self.next_id += 1;

        let mut new_shape = Shape::new(new_id);
        new_shape.parent = Some(from);
        
        // Copy properties from parent
        if let Some(parent) = self.shapes.get(&from) {
            for (name, slot) in &parent.properties {
                new_shape.properties.insert(name.clone(), *slot);
            }
        }
        
        // Add new property
        new_shape.add_property(property.to_string());

        self.shapes.insert(new_id, new_shape);
        self.transitions.insert(key, new_id);

        // Update parent's transition count
        if let Some(parent) = self.shapes.get_mut(&from) {
            parent.transitions += 1;
        }

        new_id
    }

    /// Get a shape
    pub fn get(&self, id: ShapeId) -> Option<&Shape> {
        self.shapes.get(&id)
    }

    /// Get root shape
    pub fn root() -> ShapeId {
        ShapeId(0)
    }
}

impl Default for ShapeTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ic_entry() {
        let mut entry = ICEntry::new(ShapeId(1), PropertySlot { offset: 0, is_own: true });
        
        entry.hit();
        entry.hit();
        entry.miss();
        
        assert_eq!(entry.hits, 2);
        assert_eq!(entry.misses, 1);
        assert!((entry.hit_rate() - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_polymorphic_ic() {
        let mut ic = PolymorphicIC::new();
        
        assert_eq!(ic.state(), ICState::Uninitialized);
        
        ic.add(ShapeId(1), PropertySlot { offset: 0, is_own: true });
        assert_eq!(ic.state(), ICState::Monomorphic);
        
        ic.add(ShapeId(2), PropertySlot { offset: 1, is_own: true });
        assert_eq!(ic.state(), ICState::Polymorphic);
    }

    #[test]
    fn test_ic_lookup() {
        let mut ic = PolymorphicIC::new();
        
        let slot = PropertySlot { offset: 5, is_own: true };
        ic.add(ShapeId(42), slot);
        
        assert_eq!(ic.lookup(ShapeId(42)), Some(slot));
        assert_eq!(ic.lookup(ShapeId(99)), None);
    }

    #[test]
    fn test_inline_cache_manager() {
        let mut icm = InlineCacheManager::new();
        
        let site = AccessSite::new(100, "foo");
        let shape = icm.new_shape_id();
        let slot = PropertySlot { offset: 0, is_own: true };
        
        // First access - miss
        assert!(icm.lookup(site, shape).is_none());
        
        // Record the access
        icm.record(site, shape, slot);
        
        // Second access - hit
        assert_eq!(icm.lookup(site, shape), Some(slot));
        assert!(icm.hit_rate() > 0.0);
    }

    #[test]
    fn test_shape_table() {
        let mut table = ShapeTable::new();
        
        let root = ShapeTable::root();
        let with_x = table.transition(root, "x");
        let with_xy = table.transition(with_x, "y");
        
        assert!(table.get(with_x).unwrap().has_property("x"));
        assert!(table.get(with_xy).unwrap().has_property("x"));
        assert!(table.get(with_xy).unwrap().has_property("y"));
        assert!(!table.get(with_x).unwrap().has_property("y"));
    }

    #[test]
    fn test_shape_transitions_cached() {
        let mut table = ShapeTable::new();
        
        let root = ShapeTable::root();
        let s1 = table.transition(root, "x");
        let s2 = table.transition(root, "x");
        
        // Same transition should give same shape
        assert_eq!(s1, s2);
    }
}
