//! Tagged value representation with NaN-boxing
//!
//! Uses NaN-boxing for efficient value representation:
//! - Floats: Direct IEEE 754 representation
//! - Other types: Encoded in the quiet NaN space
//!
//! Layout:
//! - Float: Any IEEE 754 double that isn't our special NaN pattern
//! - Tagged: QNAN_BASE | (tag << 47) | payload
//!
//! The quiet NaN range (exponent all 1s, quiet bit set) gives us
//! 51 bits of payload to encode other types.
//!
//! Per SPECS.md conceptual values:
//! - Nil: represented as QNAN | TAG_NIL
//! - Bool: represented as QNAN | TAG_BOOL | (0 or 1)
//! - Int: represented as QNAN | TAG_INT | (48-bit value)
//! - Pointer: represented as QNAN | TAG_PTR | (pointer data)

use std::fmt;

/// Quiet NaN base - all tagged non-float values have this pattern
/// Sign=0, Exponent=0x7FF (all 1s), Quiet bit=1
const QNAN_BASE: u64 = 0x7FFC_0000_0000_0000;

/// Mask to detect our tagged values (just check exponent + quiet bit)
/// We check bits 62-51 are all 1s (NaN with quiet bit)
const QNAN_MASK: u64 = 0x7FFC_0000_0000_0000;

/// Tag positions (bits 49-48)
const TAG_SHIFT: u32 = 48;
const TAG_MASK: u64 = 0x3;

/// Payload mask (48 bits for data)
const PAYLOAD_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

/// Type tags
const TAG_NIL: u64 = 1;
const TAG_BOOL: u64 = 2;
const TAG_INT: u64 = 3;
const TAG_PTR: u64 = 4;

/// Pointer subtypes (stored in low 3 bits of payload)
const PTR_STRING: u64 = 0;
const PTR_ARRAY: u64 = 1;
const PTR_TABLE: u64 = 2;
const PTR_CLOSURE: u64 = 3;
const PTR_FUNCTION: u64 = 4;
const PTR_USERDATA: u64 = 5;

/// A NaN-boxed value - 64 bits that can hold any Vector value
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct TaggedValue(u64);

impl TaggedValue {
    // === Constructors ===

    /// Create a nil value
    #[inline]
    pub const fn nil() -> Self {
        Self(QNAN_BASE | (TAG_NIL << TAG_SHIFT))
    }

    /// Create a boolean value
    #[inline]
    pub const fn bool(b: bool) -> Self {
        Self(QNAN_BASE | (TAG_BOOL << TAG_SHIFT) | (b as u64))
    }

    /// Create an integer value (47-bit range)
    #[inline]
    pub fn int(n: i64) -> Self {
        // Store with sign in payload
        let payload = (n as u64) & PAYLOAD_MASK;
        Self(QNAN_BASE | (TAG_INT << TAG_SHIFT) | payload)
    }

    /// Create a float value
    #[inline]
    pub fn float(f: f64) -> Self {
        // Floats are stored directly
        Self(f.to_bits())
    }

    /// Create a pointer value
    #[inline]
    pub fn ptr(p: *const u8, subtype: u64) -> Self {
        let addr = p as u64;
        // Pointers must be 8-byte aligned
        debug_assert!(addr & 0x7 == 0, "Pointer must be 8-byte aligned");
        // Encode: shift addr right by 3 (drop alignment bits), OR with subtype
        let payload = ((addr >> 3) << 3) | (subtype & 0x7);
        Self(QNAN_BASE | (TAG_PTR << TAG_SHIFT) | (payload & PAYLOAD_MASK))
    }

    /// Create a string pointer
    #[inline]
    pub fn string_ptr(p: *const u8) -> Self {
        Self::ptr(p, PTR_STRING)
    }

    /// Create an array pointer
    #[inline]
    pub fn array_ptr(p: *const u8) -> Self {
        Self::ptr(p, PTR_ARRAY)
    }

    /// Create a table pointer
    #[inline]
    pub fn table_ptr(p: *const u8) -> Self {
        Self::ptr(p, PTR_TABLE)
    }

    /// Create a closure pointer
    #[inline]
    pub fn closure_ptr(p: *const u8) -> Self {
        Self::ptr(p, PTR_CLOSURE)
    }

    /// Create a function index value
    #[inline]
    pub fn function(idx: u16) -> Self {
        Self::int((idx as i64) | (1i64 << 47))
    }

    // === Type checks ===

    /// Check if this is a tagged value (not a float)
    #[inline]
    fn is_tagged(&self) -> bool {
        // Tagged values have QNAN_MASK bits all set
        (self.0 & QNAN_MASK) == QNAN_MASK
    }

    /// Get the tag (only valid for tagged values)
    #[inline]
    fn tag(&self) -> u64 {
        (self.0 >> TAG_SHIFT) & TAG_MASK
    }

    /// Get the payload
    #[inline]
    fn payload(&self) -> u64 {
        self.0 & PAYLOAD_MASK
    }

    /// Check if this is a float (not a tagged value)
    #[inline]
    pub fn is_float(&self) -> bool {
        !self.is_tagged()
    }

    /// Check if this is nil
    #[inline]
    pub fn is_nil(&self) -> bool {
        self.is_tagged() && self.tag() == TAG_NIL
    }

    /// Check if this is a boolean
    #[inline]
    pub fn is_bool(&self) -> bool {
        self.is_tagged() && self.tag() == TAG_BOOL
    }

    /// Check if this is an integer
    #[inline]
    pub fn is_int(&self) -> bool {
        self.is_tagged() && self.tag() == TAG_INT
    }

    /// Check if this is a pointer
    #[inline]
    pub fn is_ptr(&self) -> bool {
        self.is_tagged() && self.tag() == TAG_PTR
    }

    // === Value extraction ===

    /// Extract as bool
    #[inline]
    pub fn as_bool(&self) -> Option<bool> {
        if self.is_bool() {
            Some(self.payload() != 0)
        } else {
            None
        }
    }

    /// Extract as int
    #[inline]
    pub fn as_int(&self) -> Option<i64> {
        if self.is_int() {
            let payload = self.payload();
            // Sign-extend from 48 bits
            let sign_bit = payload & (1u64 << 47);
            if sign_bit != 0 {
                Some((payload | !PAYLOAD_MASK) as i64)
            } else {
                Some(payload as i64)
            }
        } else {
            None
        }
    }

    /// Extract as float
    #[inline]
    pub fn as_float(&self) -> Option<f64> {
        if self.is_float() {
            Some(f64::from_bits(self.0))
        } else {
            None
        }
    }

    /// Extract as raw pointer
    #[inline]
    pub fn as_ptr(&self) -> Option<(*const u8, u64)> {
        if self.is_ptr() {
            let payload = self.payload();
            let subtype = payload & 0x7;
            let addr = (payload & !0x7) << 3;
            Some((addr as *const u8, subtype))
        } else {
            None
        }
    }

    /// Get pointer subtype
    #[inline]
    pub fn ptr_subtype(&self) -> Option<u64> {
        self.as_ptr().map(|(_, st)| st)
    }

    /// Check if truthy (for conditionals)
    #[inline]
    pub fn is_truthy(&self) -> bool {
        if self.is_nil() {
            false
        } else if self.is_bool() {
            self.payload() != 0
        } else {
            true
        }
    }

    /// Get type name
    pub fn type_name(&self) -> &'static str {
        if self.is_float() {
            "float"
        } else {
            match self.tag() {
                TAG_NIL => "nil",
                TAG_BOOL => "bool",
                TAG_INT => "int",
                TAG_PTR => {
                    match self.ptr_subtype() {
                        Some(PTR_STRING) => "string",
                        Some(PTR_ARRAY) => "array",
                        Some(PTR_TABLE) => "table",
                        Some(PTR_CLOSURE) => "closure",
                        Some(PTR_FUNCTION) => "function",
                        Some(PTR_USERDATA) => "userdata",
                        _ => "pointer",
                    }
                }
                _ => "unknown",
            }
        }
    }

    /// Get raw bits
    #[inline]
    pub fn bits(&self) -> u64 {
        self.0
    }

    /// Create from raw bits
    #[inline]
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }
}

impl Default for TaggedValue {
    fn default() -> Self {
        Self::nil()
    }
}

impl PartialEq for TaggedValue {
    fn eq(&self, other: &Self) -> bool {
        if self.is_float() && other.is_float() {
            f64::from_bits(self.0) == f64::from_bits(other.0)
        } else if self.is_int() && other.is_float() {
            self.as_int().unwrap() as f64 == f64::from_bits(other.0)
        } else if self.is_float() && other.is_int() {
            f64::from_bits(self.0) == other.as_int().unwrap() as f64
        } else {
            self.0 == other.0
        }
    }
}

impl Eq for TaggedValue {}

impl std::hash::Hash for TaggedValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Debug for TaggedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_nil() {
            write!(f, "Nil")
        } else if let Some(b) = self.as_bool() {
            write!(f, "Bool({})", b)
        } else if let Some(i) = self.as_int() {
            write!(f, "Int({})", i)
        } else if let Some(fl) = self.as_float() {
            write!(f, "Float({})", fl)
        } else if let Some((ptr, st)) = self.as_ptr() {
            write!(f, "Ptr({:?}, subtype={})", ptr, st)
        } else {
            write!(f, "Unknown(0x{:016X})", self.0)
        }
    }
}

impl fmt::Display for TaggedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_nil() {
            write!(f, "nil")
        } else if let Some(b) = self.as_bool() {
            write!(f, "{}", b)
        } else if let Some(i) = self.as_int() {
            write!(f, "{}", i)
        } else if let Some(fl) = self.as_float() {
            write!(f, "{}", fl)
        } else if self.is_ptr() {
            write!(f, "<{}>", self.type_name())
        } else {
            write!(f, "?")
        }
    }
}

// === Arithmetic operations ===

impl TaggedValue {
    /// Add two values
    pub fn add(&self, other: &Self) -> Option<Self> {
        match (self.as_int(), other.as_int()) {
            (Some(a), Some(b)) => Some(Self::int(a.wrapping_add(b))),
            _ => {
                let a = self.as_float().or_else(|| self.as_int().map(|i| i as f64))?;
                let b = other.as_float().or_else(|| other.as_int().map(|i| i as f64))?;
                Some(Self::float(a + b))
            }
        }
    }

    /// Subtract two values
    pub fn sub(&self, other: &Self) -> Option<Self> {
        match (self.as_int(), other.as_int()) {
            (Some(a), Some(b)) => Some(Self::int(a.wrapping_sub(b))),
            _ => {
                let a = self.as_float().or_else(|| self.as_int().map(|i| i as f64))?;
                let b = other.as_float().or_else(|| other.as_int().map(|i| i as f64))?;
                Some(Self::float(a - b))
            }
        }
    }

    /// Multiply two values
    pub fn mul(&self, other: &Self) -> Option<Self> {
        match (self.as_int(), other.as_int()) {
            (Some(a), Some(b)) => Some(Self::int(a.wrapping_mul(b))),
            _ => {
                let a = self.as_float().or_else(|| self.as_int().map(|i| i as f64))?;
                let b = other.as_float().or_else(|| other.as_int().map(|i| i as f64))?;
                Some(Self::float(a * b))
            }
        }
    }

    /// Divide two values
    pub fn div(&self, other: &Self) -> Option<Self> {
        match (self.as_int(), other.as_int()) {
            (Some(a), Some(b)) if b != 0 => Some(Self::int(a / b)),
            (Some(_), Some(_)) => None,
            _ => {
                let a = self.as_float().or_else(|| self.as_int().map(|i| i as f64))?;
                let b = other.as_float().or_else(|| other.as_int().map(|i| i as f64))?;
                Some(Self::float(a / b))
            }
        }
    }

    /// Negate
    pub fn neg(&self) -> Option<Self> {
        if let Some(i) = self.as_int() {
            Some(Self::int(-i))
        } else if let Some(f) = self.as_float() {
            Some(Self::float(-f))
        } else {
            None
        }
    }

    /// Less than
    pub fn lt(&self, other: &Self) -> Option<Self> {
        let result = match (self.as_int(), other.as_int()) {
            (Some(a), Some(b)) => a < b,
            _ => {
                let a = self.as_float().or_else(|| self.as_int().map(|i| i as f64))?;
                let b = other.as_float().or_else(|| other.as_int().map(|i| i as f64))?;
                a < b
            }
        };
        Some(Self::bool(result))
    }

    /// Less than or equal
    pub fn le(&self, other: &Self) -> Option<Self> {
        let result = match (self.as_int(), other.as_int()) {
            (Some(a), Some(b)) => a <= b,
            _ => {
                let a = self.as_float().or_else(|| self.as_int().map(|i| i as f64))?;
                let b = other.as_float().or_else(|| other.as_int().map(|i| i as f64))?;
                a <= b
            }
        };
        Some(Self::bool(result))
    }

    /// Bitwise AND
    pub fn band(&self, other: &Self) -> Option<Self> {
        Some(Self::int(self.as_int()? & other.as_int()?))
    }

    /// Bitwise OR
    pub fn bor(&self, other: &Self) -> Option<Self> {
        Some(Self::int(self.as_int()? | other.as_int()?))
    }

    /// Bitwise XOR
    pub fn bxor(&self, other: &Self) -> Option<Self> {
        Some(Self::int(self.as_int()? ^ other.as_int()?))
    }

    /// Bitwise NOT
    pub fn bnot(&self) -> Option<Self> {
        Some(Self::int(!self.as_int()?))
    }

    /// Shift left
    pub fn shl(&self, other: &Self) -> Option<Self> {
        Some(Self::int(self.as_int()? << (other.as_int()? & 63)))
    }

    /// Shift right
    pub fn shr(&self, other: &Self) -> Option<Self> {
        Some(Self::int(self.as_int()? >> (other.as_int()? & 63)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nil() {
        let v = TaggedValue::nil();
        assert!(v.is_nil());
        assert!(!v.is_truthy());
        assert_eq!(v.type_name(), "nil");
    }

    #[test]
    fn test_bool() {
        let t = TaggedValue::bool(true);
        let f = TaggedValue::bool(false);
        
        assert!(t.is_bool());
        assert!(f.is_bool());
        assert_eq!(t.as_bool(), Some(true));
        assert_eq!(f.as_bool(), Some(false));
        assert!(t.is_truthy());
        assert!(!f.is_truthy());
    }

    #[test]
    fn test_int() {
        let v = TaggedValue::int(42);
        assert!(v.is_int());
        assert_eq!(v.as_int(), Some(42));
        
        let neg = TaggedValue::int(-100);
        assert_eq!(neg.as_int(), Some(-100));
        
        let zero = TaggedValue::int(0);
        assert_eq!(zero.as_int(), Some(0));
    }

    #[test]
    fn test_float() {
        let v = TaggedValue::float(3.14);
        assert!(v.is_float());
        assert!((v.as_float().unwrap() - 3.14).abs() < 0.001);
        
        let neg = TaggedValue::float(-2.5);
        assert!(neg.is_float());
        assert!((neg.as_float().unwrap() - (-2.5)).abs() < 0.001);
    }

    #[test]
    fn test_arithmetic() {
        let a = TaggedValue::int(10);
        let b = TaggedValue::int(3);
        
        assert_eq!(a.add(&b).unwrap().as_int(), Some(13));
        assert_eq!(a.sub(&b).unwrap().as_int(), Some(7));
        assert_eq!(a.mul(&b).unwrap().as_int(), Some(30));
        assert_eq!(a.div(&b).unwrap().as_int(), Some(3));
    }

    #[test]
    fn test_float_int_mixed() {
        let i = TaggedValue::int(10);
        let f = TaggedValue::float(2.5);
        
        let result = i.add(&f).unwrap();
        assert!(result.is_float());
        assert!((result.as_float().unwrap() - 12.5).abs() < 0.001);
    }

    #[test]
    fn test_comparison() {
        let a = TaggedValue::int(5);
        let b = TaggedValue::int(10);
        
        assert_eq!(a.lt(&b).unwrap().as_bool(), Some(true));
        assert_eq!(b.lt(&a).unwrap().as_bool(), Some(false));
    }

    #[test]
    fn test_equality() {
        let a = TaggedValue::int(42);
        let b = TaggedValue::int(42);
        let c = TaggedValue::int(43);
        
        assert_eq!(a, b);
        assert_ne!(a, c);
        
        let f = TaggedValue::float(42.0);
        assert_eq!(a, f);
    }

    #[test]
    fn test_bitwise() {
        let a = TaggedValue::int(0b1010);
        let b = TaggedValue::int(0b1100);
        
        assert_eq!(a.band(&b).unwrap().as_int(), Some(0b1000));
        assert_eq!(a.bor(&b).unwrap().as_int(), Some(0b1110));
        assert_eq!(a.bxor(&b).unwrap().as_int(), Some(0b0110));
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", TaggedValue::nil()), "nil");
        assert_eq!(format!("{}", TaggedValue::bool(true)), "true");
        assert_eq!(format!("{}", TaggedValue::int(42)), "42");
    }
}
