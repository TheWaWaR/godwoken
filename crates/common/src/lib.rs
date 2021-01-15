#![cfg_attr(not(feature = "std"), no_std)]

pub mod builtin_scripts;
pub mod builtins;
pub mod error;
pub mod generator;
pub mod h256_ext;
pub mod merkle_utils;
pub mod smt;
pub mod state;
pub mod sudt;
pub mod traits;

// re-exports

pub use generator::ChallengeContext;
pub use generator::RunResult;
pub use gw_hash::blake2b;
pub use h256_ext::H256;
pub use sparse_merkle_tree;

/// Common constants

pub const FINALIZE_BLOCKS: u64 = 1000;

pub const DEPOSITION_LOCK_CODE_HASH: [u8; 32] = [0u8; 32];
pub const CUSTODIAN_LOCK_CODE_HASH: [u8; 32] = [0u8; 32];
pub const L2_SUDT_CODE_HASH: [u8; 32] = [0u8; 32];
pub const CKB_SUDT_SCRIPT_HASH: [u8; 32] = [
    128, 74, 52, 101, 132, 195, 173, 228, 233, 202, 88, 4, 18, 108, 212, 244, 241, 77, 210, 5, 153,
    202, 161, 219, 140, 187, 63, 65, 168, 184, 176, 129,
];
pub const CKB_SUDT_SCRIPT_ARGS: [u8; 32] = [0; 32];
pub const ACCOUNT_LOCK_CODE_HASH: [u8; 32] = [0u8; 32];
pub const ROLLUP_LOCK_CODE_HASH: [u8; 32] = [0u8; 32];

pub fn code_hash(data: &[u8]) -> H256 {
    let mut hasher = blake2b::new_blake2b();
    hasher.update(data);
    let mut code_hash = [0u8; 32];
    hasher.finalize(&mut code_hash);
    code_hash.into()
}

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        use std::vec;
    } else {
        extern crate alloc;
        use alloc::vec;
    }
}
