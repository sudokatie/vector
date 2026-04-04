//! Userdata - opaque handles to host data
//!
//! Allows embedding Rust data in Vector scripts.

use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use crate::vm::{Value, RuntimeError};

/// Trait for userdata types
pub trait UserdataType: Any + 'static {
    /// Type name shown in error messages
    fn type_name() -> &'static str where Self: Sized;
    
    /// Optional: called when userdata is garbage collected
    fn finalize(&mut self) {}
}

/// Builder for creating userdata values
pub struct UserdataBuilder;

impl UserdataBuilder {
    /// Create a new userdata value wrapping the given Rust data
    pub fn create<T: UserdataType>(data: T) -> Value {
        let ud = crate::vm::value::Userdata {
            data: Box::new(data),
            type_name: T::type_name(),
        };
        Value::Userdata(Rc::new(RefCell::new(ud)))
    }
}

/// Helper to check if userdata matches a type
pub fn check_userdata_type<T: UserdataType + 'static>(value: &Value) -> Result<(), RuntimeError> {
    match value {
        Value::Userdata(ud) => {
            let borrowed = ud.borrow();
            if borrowed.type_name == T::type_name() {
                Ok(())
            } else {
                Err(RuntimeError::TypeError {
                    expected: T::type_name().to_string(),
                    got: borrowed.type_name.to_string(),
                })
            }
        }
        v => Err(RuntimeError::TypeError {
            expected: T::type_name().to_string(),
            got: v.type_name().to_string(),
        }),
    }
}

/// Helper to extract typed userdata from a Value
/// Returns a Ref that can be used to access the data
pub fn get_userdata<T: UserdataType + 'static>(value: &Value) -> Result<std::cell::Ref<'_, T>, RuntimeError> {
    match value {
        Value::Userdata(ud) => {
            check_userdata_type::<T>(value)?;
            let borrowed = ud.borrow();
            Ok(std::cell::Ref::map(borrowed, |ud| {
                ud.data.downcast_ref::<T>().expect("Type already verified")
            }))
        }
        v => Err(RuntimeError::TypeError {
            expected: T::type_name().to_string(),
            got: v.type_name().to_string(),
        }),
    }
}

/// Helper to extract typed mutable userdata from a Value
pub fn get_userdata_mut<T: UserdataType + 'static>(value: &Value) -> Result<std::cell::RefMut<'_, T>, RuntimeError> {
    match value {
        Value::Userdata(ud) => {
            check_userdata_type::<T>(value)?;
            let borrowed = ud.borrow_mut();
            Ok(std::cell::RefMut::map(borrowed, |ud| {
                ud.data.downcast_mut::<T>().expect("Type already verified")
            }))
        }
        v => Err(RuntimeError::TypeError {
            expected: T::type_name().to_string(),
            got: v.type_name().to_string(),
        }),
    }
}

/// Check if a value is userdata of a specific type
pub fn is_userdata<T: UserdataType + 'static>(value: &Value) -> bool {
    match value {
        Value::Userdata(ud) => ud.borrow().type_name == T::type_name(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestData {
        value: i32,
    }

    impl UserdataType for TestData {
        fn type_name() -> &'static str {
            "TestData"
        }
    }

    #[test]
    fn test_create_userdata() {
        let ud = UserdataBuilder::create(TestData { value: 42 });
        assert_eq!(ud.type_name(), "TestData");
    }

    #[test]
    fn test_is_userdata() {
        let ud = UserdataBuilder::create(TestData { value: 42 });
        assert!(is_userdata::<TestData>(&ud));
        
        let not_ud = Value::Int(42);
        assert!(!is_userdata::<TestData>(&not_ud));
    }
}
