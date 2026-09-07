//! Repository release management. Product versions are independent of wire schemas.
mod artifacts;
mod json_edit;
mod versions;

pub use artifacts::{bundle, checksums, release_targets};
pub use versions::{check, notes, prepare, read_version, sync};

#[cfg(test)]
mod tests;
