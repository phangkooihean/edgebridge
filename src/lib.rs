//! # EdgeBridge Core Library (`v1.0.0-rc.1`)
//!
//! Cross-platform Rust library powering EdgeBridge across **macOS**,
//! **Ubuntu/Debian**, **Fedora**, and **native Windows** with multi-peer
//! topology, shared cross-node telemetry, declarative Rust macros, and
//! invariant unit tests coupled with `///` documentation tests.

#[macro_use]
pub mod macros;
pub mod kvm;
pub mod models;
pub mod platform;
pub mod power;
pub mod reclaim;
pub mod scripts;
pub mod telemetry;
pub mod transport;
pub mod web_api;
