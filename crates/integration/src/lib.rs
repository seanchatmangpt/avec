//! Cross-crate integration harness for the cgap workspace.
//!
//! The suites live in `tests/` and drive the real `cgap-arrow` collaborators
//! (builders, C Data Interface structs, aligned buffers) with assertions on
//! final memory state — Chicago discipline, no mocks.
