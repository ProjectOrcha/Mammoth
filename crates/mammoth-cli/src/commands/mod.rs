//! One module per command group. Each takes `&dyn Backend` and returns a type
//! that implements [`crate::output::Render`], so a command never knows whether
//! it is talking to `LocalBackend` or a real cluster, and never decides how its
//! output is formatted.
//!
//! Populate in roadmap order: `fs` and `viz` first (M1–M2), the rest after.

// pub mod admin;
// pub mod bench;
// pub mod cluster;
// pub mod config;
// pub mod doctor;
// pub mod fs;
// pub mod job;
// pub mod migrate;
// pub mod node;
// pub mod quickstart;
// pub mod serve;
// pub mod top;
// pub mod viz;
