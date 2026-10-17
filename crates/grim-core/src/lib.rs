pub mod arz;
pub mod desktop;
pub mod discovery;
pub mod game_data;
#[cfg(feature = "legacy-tools")]
pub mod gdc;
pub mod live;
pub mod model;
#[cfg(feature = "legacy-tools")]
pub mod rules;
#[cfg(feature = "legacy-tools")]
pub mod service;
pub mod storage;

pub fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
