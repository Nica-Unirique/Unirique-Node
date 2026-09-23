use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Busy(pub Arc<AtomicUsize>);

impl Drop for Busy {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}