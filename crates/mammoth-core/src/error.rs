//! Errors that teach.
//!
//! Design principle 3 from Part V: never print a stack trace. Every variant
//! carries a stable `E….` code so the CLI can point at
//! `https://sakib-dalal.github.io/mammoth/errors/<code>` and so scripts can
//! match on something that will not change when the wording does.

use std::path::PathBuf;

/// Convenience alias used throughout the workspace.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Everything that can go wrong at the `Backend` boundary.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The path does not exist in the namespace.
    #[error("no such path: {0}")]
    NotFound(PathBuf),

    /// The path exists but is the wrong kind for this operation.
    #[error("{path} is a {actual}, expected a {expected}")]
    WrongKind {
        /// The offending path.
        path: PathBuf,
        /// What it actually is.
        actual: &'static str,
        /// What the caller needed.
        expected: &'static str,
    },

    /// Fewer live workers than the requested replication factor. `E0301`.
    #[error("not enough healthy workers for replication {wanted}: only {available} available")]
    NotEnoughWorkers {
        /// Replicas the write asked for.
        wanted: u8,
        /// Workers currently able to accept a replica.
        available: u8,
    },

    /// The master is still rebuilding its block map and will not accept writes.
    #[error("cluster is in safe mode: {reported:.3} of blocks reported, need {threshold:.3}")]
    SafeMode {
        /// Fraction of blocks accounted for so far.
        reported: f64,
        /// Fraction required by `master.safemode_threshold`.
        threshold: f64,
    },

    /// Checksum verification failed on read or on a pipeline hop.
    #[error("checksum mismatch on {path}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        /// The file being read.
        path: PathBuf,
        /// Checksum recorded at write time.
        expected: String,
        /// Checksum computed now.
        actual: String,
    },

    /// Another client holds the write lease on this file.
    #[error("{path} is open for writing by {holder}")]
    LeaseHeld {
        /// The file.
        path: PathBuf,
        /// Current lease holder.
        holder: String,
    },

    /// Configuration failed to load or validate.
    #[error("config error: {0}")]
    Config(String),

    /// Transport or I/O failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Error {
    /// Stable error code, for `--json` output and for the docs URL.
    pub fn code(&self) -> &'static str {
        match self {
            Error::NotFound(_) => "E0101",
            Error::WrongKind { .. } => "E0102",
            Error::NotEnoughWorkers { .. } => "E0301",
            Error::SafeMode { .. } => "E0302",
            Error::ChecksumMismatch { .. } => "E0401",
            Error::LeaseHeld { .. } => "E0201",
            Error::Config(_) => "E0001",
            Error::Io(_) => "E0500",
        }
    }

    /// Concrete next commands to suggest, in the order a user should try them.
    pub fn hints(&self) -> Vec<String> {
        match self {
            Error::NotEnoughWorkers { available, .. } => vec![
                format!("lower replication:   mammoth put <src> <dst> --replication {available}"),
                "check node health:   mammoth node list".into(),
                "why is a node down:  mammoth doctor --node <id>".into(),
            ],
            Error::SafeMode { .. } => vec![
                "watch progress:      mammoth admin safemode status".into(),
                "force leave (risky): mammoth admin safemode leave --force".into(),
            ],
            Error::LeaseHeld { .. } => {
                vec!["inspect open files:  mammoth admin report --leases".into()]
            }
            Error::Config(_) => vec!["validate config:     mammoth config validate".into()],
            _ => Vec::new(),
        }
    }

    /// Documentation URL for this error code.
    pub fn docs_url(&self) -> String {
        format!("https://sakib-dalal.github.io/mammoth/errors/{}", self.code())
    }
}
