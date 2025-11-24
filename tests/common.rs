use std::sync::{Mutex, MutexGuard, OnceLock};

/// Global mutex to serialize integration tests that share mutable fixtures.
pub fn serial() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}
