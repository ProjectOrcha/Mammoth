# ADR 0004: durable local service and explicit capability reporting

The guided M1–M3 product needs real storage behind the CLI and dashboard. The
six workers remain directories on one host, with rack-aware rendezvous placement.
This is a local service; it does not establish distributed consensus or HA.

Use an atomically replaced namespace snapshot and an OS file lock shared by all
processes opening the store. Immutable block directories contain data and CRC32C
checksums for each 4 KiB chunk. Sync files before publishing and sync the parent
directory on Unix. Publish data before metadata; reclaim unreferenced blocks only
after the metadata commit. Interrupted writes may leave unreferenced blocks but
must not expose partial files. An explicit garbage collection pass reclaims them.

Stage input and range reads through temporary files, keeping memory bounded by
one configured block and avoiding filesystem work on async runtime threads. This
costs extra local disk I/O and is not the future direct-worker streaming path.

Normalize namespace paths independently of host paths; reject parent traversal,
NULs and backslashes. Namespace metadata is separate from user filenames, so
names ending in `.mmeta` cannot collide with internal metadata.

The gateway adapts core records to dashboard records, converts timestamps to
milliseconds and exposes capabilities. Unsupported distributed metrics and
historical replay must be visibly unavailable, never fabricated measurements.
S3 support is an explicitly documented subset, served on a separate listener.
