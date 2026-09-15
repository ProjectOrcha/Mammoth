//! LocalBackend — single-machine simulation of a cluster
//!
//! `gfs` is an executable, in-memory GFS teaching model. It is not a Backend.
//! LocalBackend is not yet implemented — scheduled for milestone M1.
//! Build it by following `docs/guide/05-localbackend-part-1.md` and
//! `docs/guide/06-localbackend-part-2.md`.

#![forbid(unsafe_code)]

pub mod gfs;
