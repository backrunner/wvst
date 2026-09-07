//! Repository release management. Product versions are independent of wire schemas.
mod artifacts;
mod json_edit;
mod macos;
mod versions;

pub use artifacts::{bundle, checksums, release_targets};
pub use macos::{bundle_macos, store_notary_credentials, verify_macos_signatures};
pub use versions::{check, notes, prepare, read_version, sync};

#[cfg(test)]
mod tests;
