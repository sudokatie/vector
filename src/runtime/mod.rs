//! Runtime support (strings, arrays, tables, stdlib)

pub mod string;
pub mod array;
pub mod table;
pub mod stdlib;
pub mod intern;
pub mod userdata;

pub use stdlib::*;
pub use intern::{InternedString, StringInterner, intern, intern_owned, interner_stats};
pub use userdata::{UserdataType, UserdataBuilder, get_userdata, get_userdata_mut, is_userdata};
