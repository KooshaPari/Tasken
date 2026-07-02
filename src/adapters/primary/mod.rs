// SPDX-License-Identifier: MIT OR Apache-2.0
//! Primary adapters - entry points into the domain.

pub mod cli;
pub mod color_choice;

pub use cli::CliAdapter;
pub use color_choice::ColorChoice;
