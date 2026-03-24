//! JIT compilation via Cranelift

pub mod profile;

/// JIT compilation coordinator
pub struct Jit {
    enabled: bool,
}

impl Jit {
    pub fn new() -> Self {
        Self { enabled: true }
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for Jit {
    fn default() -> Self {
        Self::new()
    }
}
