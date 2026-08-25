//! Core vocabulary for Mammoth.
//!
//! Every other crate depends on this one and nothing here depends on a
//! transport, a runtime, or a storage layout. The whole architecture hangs off
//! [`Backend`]: `mammoth-local` implements it against one machine's disk,
//! `mammoth-client` implements it against a real cluster, and the CLI and the
//! gateway are written against the trait so they never learn the difference.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod backend;
pub mod config;
pub mod error;
pub mod types;

pub use backend::Backend;
pub use error::{Error, Result};
pub use types::{
    BlockId, BlockPlacement, ClusterReport, FileStatus, NodeId, NodeState, Replica, ReplicaState,
};
