//! Roblox property types and serialization
//!
//! This module defines all Roblox property types and their JSON representations.
//! The goal is to capture every possible property value with full fidelity.

mod properties;
mod instance;
mod project;
mod wally;
mod harness;

pub use properties::*;
pub use instance::*;
pub use project::*;
pub use wally::*;
pub use harness::*;
