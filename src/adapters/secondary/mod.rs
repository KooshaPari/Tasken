// SPDX-License-Identifier: MIT OR Apache-2.0
//! Secondary adapters - external system integrations.

pub mod file;
pub mod memory;

pub use file::FileStorage;
pub use memory::MemoryStorage;
