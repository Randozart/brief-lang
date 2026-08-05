// 2026-07-25: Beastpack binary serialization module.
// Wraps the existing BEAST text format in a binary container with
// compression, checksums, and metadata stripping for distribution.
//
// The .beastpack file is the portable distribution format for
// Briv's install-time compilation pipeline. It contains the typed
// AST after the $(Typed) pipeline stage, with Source$/Comment$
// metadata stripped and internal identifiers optionally obfuscated.
//
// See docs/architecture/bounty-architecture.md for the full spec.

pub mod serialize;
pub mod deserialize;
pub mod strip;
pub mod obfuscate;

#[cfg(test)]
mod tests;

pub use serialize::serialize;
pub use deserialize::deserialize;
