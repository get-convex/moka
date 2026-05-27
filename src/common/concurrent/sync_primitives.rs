// Shim module that re-exports either std/portable_atomic or shuttle-aware
// atomic types, selected at compile time by `cfg(moka_shuttle)`.
//
// parking_lot types (Mutex, RwLock, etc.) do NOT need shims here because the
// `parking_lot` dependency is declared as `{ package = "shuttle-parking_lot" }`
// in Cargo.toml. When the `shuttle-testing` feature is active, the
// `parking_lot/shuttle` feature is also activated, which makes every
// `use parking_lot::*` import automatically use shuttle-aware implementations.

#[cfg(not(moka_shuttle))]
pub(crate) use std::sync::atomic::{
    self as atomic, AtomicBool, AtomicU16, AtomicU32, AtomicU8, Ordering,
};

#[cfg(not(moka_shuttle))]
pub(crate) use portable_atomic::AtomicU64;

#[cfg(moka_shuttle)]
pub(crate) use shuttle::sync::atomic::{
    self as atomic, AtomicBool, AtomicU16, AtomicU32, AtomicU8, AtomicU64, Ordering,
};
