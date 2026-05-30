// Shims for synchronization primitives selected by `cfg(moka_shuttle)`.
//
// For non-shuttle builds we keep using `parking_lot` locks and std/portable
// atomics. For shuttle builds we keep the lock API surface mostly compatible
// with `parking_lot` while routing operations through shuttle primitives.

#[cfg(not(moka_shuttle))]
pub(crate) use std::sync::atomic::{
    self as atomic, AtomicBool, AtomicU16, AtomicU32, AtomicU8, Ordering,
};

#[cfg(not(moka_shuttle))]
pub(crate) use portable_atomic::AtomicU64;

#[cfg(moka_shuttle)]
pub(crate) use shuttle::sync::atomic::{
    self as atomic, AtomicBool, AtomicU16, AtomicU32, AtomicU64, AtomicU8, Ordering,
};

#[cfg(not(moka_shuttle))]
pub(crate) use parking_lot::{Mutex, MutexGuard, RwLock};

#[cfg(moka_shuttle)]
mod shuttle_lock_impl {
    use std::sync::TryLockError;

    pub(crate) type MutexGuard<'a, T> = shuttle::sync::MutexGuard<'a, T>;
    type RwLockReadGuard<'a, T> = shuttle::sync::RwLockReadGuard<'a, T>;
    type RwLockWriteGuard<'a, T> = shuttle::sync::RwLockWriteGuard<'a, T>;

    pub(crate) struct Mutex<T>(shuttle::sync::Mutex<T>);

    impl<T> Mutex<T> {
        pub(crate) fn new(value: T) -> Self {
            Self(shuttle::sync::Mutex::new(value))
        }

        pub(crate) fn lock(&self) -> MutexGuard<'_, T> {
            self.0.lock().unwrap()
        }

        pub(crate) fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
            match self.0.try_lock() {
                Ok(guard) => Some(guard),
                Err(TryLockError::WouldBlock) => None,
                Err(TryLockError::Poisoned(err)) => Some(err.into_inner()),
            }
        }
    }

    impl<T: Default> Default for Mutex<T> {
        fn default() -> Self {
            Self::new(T::default())
        }
    }

    pub(crate) struct RwLock<T>(shuttle::sync::RwLock<T>);

    impl<T> RwLock<T> {
        pub(crate) fn new(value: T) -> Self {
            Self(shuttle::sync::RwLock::new(value))
        }

        pub(crate) fn read(&self) -> RwLockReadGuard<'_, T> {
            self.0.read().unwrap()
        }

        pub(crate) fn write(&self) -> RwLockWriteGuard<'_, T> {
            self.0.write().unwrap()
        }
    }

    impl<T: Default> Default for RwLock<T> {
        fn default() -> Self {
            Self::new(T::default())
        }
    }
}

#[cfg(moka_shuttle)]
pub(crate) use shuttle_lock_impl::{Mutex, MutexGuard, RwLock};
