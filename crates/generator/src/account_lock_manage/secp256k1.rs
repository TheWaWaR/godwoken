use super::LockAlgorithm;
use crate::{error::LockAlgorithmError, RollupContext};
use gw_common::blake2b::new_blake2b;
use gw_common::H256;
use gw_types::prelude::*;
use gw_types::{
    bytes::Bytes,
    packed::{L2Transaction, RawL2Transaction, Script, Signature},
};
use lazy_static::lazy_static;
use secp256k1::recovery::{RecoverableSignature, RecoveryId};
use sha3::{Digest, Keccak256};

lazy_static! {
    pub static ref SECP256K1: secp256k1::Secp256k1<secp256k1::All> = secp256k1::Secp256k1::new();
}

#[derive(Debug, Default)]
pub struct Secp256k1;

/// Usage
/// register an algorithm to AccountLockManage
///
/// manage.register_lock_algorithm(code_hash, Box::new(AlwaysSuccess::default()));
impl LockAlgorithm for Secp256k1 {
    fn verify_tx(
        &self,
        ctx: &RollupContext,
        sender_script: Script,
        receiver_script: Script,
        tx: L2Transaction,
    ) -> Result<bool, LockAlgorithmError> {
        let message = calc_godwoken_signing_message(
            &ctx.rollup_script_hash,
            &sender_script,
            &receiver_script,
            &tx,
        );

        self.verify_withdrawal_signature(sender_script.args().unpack(), tx.signature(), message)
    }

    fn verify_withdrawal_signature(
        &self,
        lock_args: Bytes,
        signature: Signature,
        message: H256,
    ) -> Result<bool, LockAlgorithmError> {
        if lock_args.len() != 52 {
            return Err(LockAlgorithmError::InvalidLockArgs);
        }
        let mut expected_pubkey_hash = [0u8; 20];
        expected_pubkey_hash.copy_from_slice(&lock_args[32..52]);
        let signature: RecoverableSignature = {
            let signature: [u8; 65] = signature.unpack();
            let recid = RecoveryId::from_i32(signature[64] as i32)
                .map_err(|_| LockAlgorithmError::InvalidSignature)?;
            let data = &signature[..64];
            RecoverableSignature::from_compact(data, recid)
                .map_err(|_| LockAlgorithmError::InvalidSignature)?
        };
        let msg = secp256k1::Message::from_slice(message.as_slice())
            .map_err(|_| LockAlgorithmError::InvalidSignature)?;
        let pubkey = SECP256K1
            .recover(&msg, &signature)
            .map_err(|_| LockAlgorithmError::InvalidSignature)?;
        let pubkey_hash = {
            let mut buf = [0u8; 32];
            let mut hasher = new_blake2b();
            hasher.update(&pubkey.serialize());
            hasher.finalize(&mut buf);
            let mut pubkey_hash = [0u8; 20];
            pubkey_hash.copy_from_slice(&buf[..20]);
            pubkey_hash
        };
        if pubkey_hash != expected_pubkey_hash {
            return Ok(false);
        }
        Ok(true)
    }
}

#[derive(Debug, Default)]
pub struct Secp256k1Eth;

impl Secp256k1Eth {
    fn verify_alone(
        &self,
        lock_args: Bytes,
        signature: Signature,
        message: H256,
    ) -> Result<bool, LockAlgorithmError> {
        if lock_args.len() != 52 {
            return Err(LockAlgorithmError::InvalidLockArgs);
        }

        let mut expected_pubkey_hash = [0u8; 20];
        expected_pubkey_hash.copy_from_slice(&lock_args[32..52]);
        let signature: RecoverableSignature = {
            let signature: [u8; 65] = signature.unpack();
            let recid = RecoveryId::from_i32(signature[64] as i32)
                .map_err(|_| LockAlgorithmError::InvalidSignature)?;
            let data = &signature[..64];
            RecoverableSignature::from_compact(data, recid)
                .map_err(|_| LockAlgorithmError::InvalidSignature)?
        };
        let msg = secp256k1::Message::from_slice(message.as_slice())
            .map_err(|_| LockAlgorithmError::InvalidSignature)?;
        let pubkey = SECP256K1
            .recover(&msg, &signature)
            .map_err(|_| LockAlgorithmError::InvalidSignature)?;
        let pubkey_hash = {
            let mut hasher = Keccak256::new();
            hasher.update(&pubkey.serialize_uncompressed()[1..]);
            let buf = hasher.finalize();
            let mut pubkey_hash = [0u8; 20];
            pubkey_hash.copy_from_slice(&buf[12..]);
            pubkey_hash
        };
        if pubkey_hash != expected_pubkey_hash {
            return Ok(false);
        }
        Ok(true)
    }
}

/// Usage
/// register AlwaysSuccess to AccountLockManage
///
/// manage.register_lock_algorithm(code_hash, Box::new(AlwaysSuccess::default()));
impl LockAlgorithm for Secp256k1Eth {
    fn verify_tx(
        &self,
        ctx: &RollupContext,
        sender_script: Script,
        receiver_script: Script,
        tx: L2Transaction,
    ) -> Result<bool, LockAlgorithmError> {
        if let Some(rlp_data) = try_assemble_polyjuice_args(
            ctx.rollup_config.compatible_chain_id().unpack(),
            tx.raw(),
            receiver_script.clone(),
        ) {
            let mut hasher = Keccak256::new();
            hasher.update(&rlp_data);
            let buf = hasher.finalize();
            let mut signing_message = [0u8; 32];
            signing_message.copy_from_slice(&buf[..]);
            let signing_message = H256::from(signing_message);
            return self.verify_alone(
                sender_script.args().unpack(),
                tx.signature(),
                signing_message,
            );
        }

        let message = calc_godwoken_signing_message(
            &ctx.rollup_script_hash,
            &sender_script,
            &receiver_script,
            &tx,
        );
        self.verify_withdrawal_signature(sender_script.args().unpack(), tx.signature(), message)
    }

    // NOTE: verify_tx in this module is using standard Ethereum transaction
    // signing scheme, but verify_withdrawal_signature here is using Ethereum's
    // personal sign(with "\x19Ethereum Signed Message:\n32" appended),
    // this is because verify_tx is designed to provide seamless compatibility
    // with Ethereum, but withdrawal request is a godwoken thing, which
    // do not exist in Ethereum. Personal sign is thus used here.
    fn verify_withdrawal_signature(
        &self,
        lock_args: Bytes,
        signature: Signature,
        message: H256,
    ) -> Result<bool, LockAlgorithmError> {
        let mut hasher = Keccak256::new();
        hasher.update("\x19Ethereum Signed Message:\n32");
        hasher.update(message.as_slice());
        let buf = hasher.finalize();
        let mut signing_message = [0u8; 32];
        signing_message.copy_from_slice(&buf[..]);
        let signing_message = H256::from(signing_message);

        self.verify_alone(lock_args, signature, signing_message)
    }
}

#[derive(Debug, Default)]
pub struct Secp256k1Tron;

/// Usage
/// register Secp256k1Tron to AccountLockManage
///
/// manage.register_lock_algorithm(code_hash, Box::new(Secp256k1Tron::default()));
impl LockAlgorithm for Secp256k1Tron {
    fn verify_tx(
        &self,
        ctx: &RollupContext,
        sender_script: Script,
        receiver_script: Script,
        tx: L2Transaction,
    ) -> Result<bool, LockAlgorithmError> {
        let message = calc_godwoken_signing_message(
            &ctx.rollup_script_hash,
            &sender_script,
            &receiver_script,
            &tx,
        );

        self.verify_withdrawal_signature(sender_script.args().unpack(), tx.signature(), message)
    }

    fn verify_withdrawal_signature(
        &self,
        lock_args: Bytes,
        signature: Signature,
        message: H256,
    ) -> Result<bool, LockAlgorithmError> {
        if lock_args.len() != 52 {
            return Err(LockAlgorithmError::InvalidLockArgs);
        }
        let mut hasher = Keccak256::new();
        hasher.update("\x19TRON Signed Message:\n32");
        hasher.update(message.as_slice());
        let buf = hasher.finalize();
        let mut signing_message = [0u8; 32];
        signing_message.copy_from_slice(&buf[..]);
        let signing_message = H256::from(signing_message);
        let mut expected_pubkey_hash = [0u8; 20];
        expected_pubkey_hash.copy_from_slice(&lock_args[32..52]);
        let signature: RecoverableSignature = {
            let signature: [u8; 65] = signature.unpack();
            let recid = {
                let rec_param: i32 = match signature[64] {
                    28 => 1,
                    _ => 0,
                };
                RecoveryId::from_i32(rec_param).map_err(|_| LockAlgorithmError::InvalidSignature)?
            };
            let data = &signature[..64];
            RecoverableSignature::from_compact(data, recid)
                .map_err(|_| LockAlgorithmError::InvalidSignature)?
        };
        let msg = secp256k1::Message::from_slice(signing_message.as_slice())
            .map_err(|_| LockAlgorithmError::InvalidSignature)?;
        let pubkey = SECP256K1
            .recover(&msg, &signature)
            .map_err(|_| LockAlgorithmError::InvalidSignature)?;
        let pubkey_hash = {
            let mut hasher = Keccak256::new();
            hasher.update(&pubkey.serialize_uncompressed()[1..]);
            let buf = hasher.finalize();
            let mut pubkey_hash = [0u8; 20];
            pubkey_hash.copy_from_slice(&buf[12..]);
            pubkey_hash
        };
        if pubkey_hash != expected_pubkey_hash {
            return Ok(false);
        }
        Ok(true)
    }
}

fn calc_godwoken_signing_message(
    rollup_type_hash: &H256,
    sender_script: &Script,
    receiver_script: &Script,
    tx: &L2Transaction,
) -> H256 {
    tx.raw().calc_message(
        &rollup_type_hash,
        &sender_script.hash().into(),
        &receiver_script.hash().into(),
    )
}

fn try_assemble_polyjuice_args(
    rollup_chain_id: u32,
    raw_tx: RawL2Transaction,
    receiver_script: Script,
) -> Option<Bytes> {
    let args: Bytes = raw_tx.args().unpack();
    if args.len() < 52 {
        return None;
    }
    if args[0..7] != b"\xFF\xFF\xFFPOLY"[..] {
        return None;
    }
    let mut stream = rlp::RlpStream::new();
    stream.begin_unbounded_list();
    let nonce: u32 = raw_tx.nonce().unpack();
    stream.append(&nonce);
    let gas_price = {
        let mut data = [0u8; 16];
        data.copy_from_slice(&args[16..32]);
        u128::from_le_bytes(data)
    };
    stream.append(&gas_price);
    let gas_limit = {
        let mut data = [0u8; 8];
        data.copy_from_slice(&args[8..16]);
        u64::from_le_bytes(data)
    };
    stream.append(&gas_limit);
    let (to, polyjuice_chain_id) = if args[7] == 3 {
        // 3 for EVMC_CREATE
        // In case of deploying a polyjuice contract, to id(creator account id)
        // is directly used as chain id
        (vec![0u8; 0], raw_tx.to_id().unpack())
    } else {
        // For contract calling, chain id is read from scrpit args of
        // receiver_script, see the following link for more details:
        // https://github.com/nervosnetwork/godwoken-polyjuice#normal-contract-account-script
        if receiver_script.args().len() < 36 {
            return None;
        }
        let polyjuice_chain_id = {
            let mut data = [0u8; 4];
            data.copy_from_slice(&receiver_script.args().raw_data()[32..36]);
            u32::from_le_bytes(data)
        };
        let mut to = vec![0u8; 20];
        let receiver_hash = receiver_script.hash();
        to[0..16].copy_from_slice(&receiver_hash[0..16]);
        let to_id: u32 = raw_tx.to_id().unpack();
        to[16..20].copy_from_slice(&to_id.to_le_bytes());
        (to, polyjuice_chain_id)
    };
    stream.append(&to);
    let value = {
        let mut data = [0u8; 16];
        data.copy_from_slice(&args[32..48]);
        u128::from_le_bytes(data)
    };
    stream.append(&value);
    let payload_length = {
        let mut data = [0u8; 4];
        data.copy_from_slice(&args[48..52]);
        u32::from_le_bytes(data)
    } as usize;
    if args.len() != 52 + payload_length {
        return None;
    }
    stream.append(&args[52..52 + payload_length].to_vec());
    let chain_id: u64 = ((rollup_chain_id as u64) << 32) | (polyjuice_chain_id as u64);
    stream.append(&chain_id);
    stream.append(&0u8);
    stream.append(&0u8);
    stream.finalize_unbounded_list();
    Some(Bytes::from(stream.out().to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secp256k1_eth_withdrawal_signature() {
        let message = H256::from([0u8; 32]);
        let test_signature = Signature::from_slice(
        &hex::decode("c2ae67217b65b785b1add7db1e9deb1df2ae2c7f57b9c29de0dfc40c59ab8d47341a863876660e3d0142b71248338ed71d2d4eb7ca078455565733095ac25a5800").expect("hex decode"))
        .expect("create signature structure");
        let address = Bytes::from(
            hex::decode("ffafb3db9377769f5b59bfff6cd2cf942a34ab17").expect("hex decode"),
        );
        let mut lock_args = vec![0u8; 32];
        lock_args.extend(address);
        let eth = Secp256k1Eth {};
        let result = eth
            .verify_withdrawal_signature(lock_args.into(), test_signature, message)
            .expect("verify signature");
        assert!(result);
    }

    #[test]
    fn test_secp256k1_eth_polyjuice_call() {
        let mut polyjuice_args = vec![0u8; 52];
        polyjuice_args[0..7].copy_from_slice(b"\xFF\xFF\xFFPOLY");
        polyjuice_args[7] = 0;
        let gas_limit: u64 = 21000;
        polyjuice_args[8..16].copy_from_slice(&gas_limit.to_le_bytes());
        let gas_price: u128 = 20000000000;
        polyjuice_args[16..32].copy_from_slice(&gas_price.to_le_bytes());
        let value: u128 = 3000000;
        polyjuice_args[32..48].copy_from_slice(&value.to_le_bytes());
        let payload_length: u32 = 0;
        polyjuice_args[48..52].copy_from_slice(&payload_length.to_le_bytes());

        let raw_tx = RawL2Transaction::new_builder()
            .nonce(9u32.pack())
            .to_id(1234u32.pack())
            .args(Bytes::from(polyjuice_args).pack())
            .build();
        let mut signature = [0u8; 65];
        signature.copy_from_slice(&hex::decode("239ff31262bb6664d1857ea3bc5eecf3a4f74e32537c81de9fa1df2a2a48ef63115ffd8d6f5b4cc60b0fd4b02ab641106d024e49a9c0a9657c99361b39ce31ec00").expect("hex decode"));
        let signature = Signature::from_slice(&signature[..]).unwrap();
        let tx = L2Transaction::new_builder()
            .raw(raw_tx)
            .signature(signature)
            .build();
        let eth = Secp256k1Eth {};

        let rollup_type_hash = vec![0u8; 32];

        let mut sender_args = vec![];
        sender_args.extend(&rollup_type_hash);
        sender_args
            .extend(&hex::decode("9d8A62f656a8d1615C1294fd71e9CFb3E4855A4F").expect("hex decode"));
        let sender_script = Script::new_builder()
            .args(Bytes::from(sender_args).pack())
            .build();

        let mut receiver_args = vec![];
        receiver_args.extend(&rollup_type_hash);
        receiver_args.extend(&23u32.to_le_bytes());
        let receiver_script = Script::new_builder()
            .args(Bytes::from(receiver_args).pack())
            .build();
        let ctx = RollupContext {
            rollup_script_hash: Default::default(),
            rollup_config: Default::default(),
        };
        let result = eth
            .verify_tx(&ctx, sender_script, receiver_script, tx)
            .expect("verify signature");
        assert!(result);
    }

    #[test]
    fn test_secp256k1_eth_polyjuice_call_with_to_containing_leading_zeros() {
        let mut polyjuice_args = vec![0u8; 52];
        polyjuice_args[0..7].copy_from_slice(b"\xFF\xFF\xFFPOLY");
        polyjuice_args[7] = 0;
        let gas_limit: u64 = 21000;
        polyjuice_args[8..16].copy_from_slice(&gas_limit.to_le_bytes());
        let gas_price: u128 = 20000000000;
        polyjuice_args[16..32].copy_from_slice(&gas_price.to_le_bytes());
        let value: u128 = 3000000;
        polyjuice_args[32..48].copy_from_slice(&value.to_le_bytes());
        let payload_length: u32 = 0;
        polyjuice_args[48..52].copy_from_slice(&payload_length.to_le_bytes());

        let raw_tx = RawL2Transaction::new_builder()
            .nonce(9u32.pack())
            .to_id(1234u32.pack())
            .args(Bytes::from(polyjuice_args).pack())
            .build();
        let mut signature = [0u8; 65];
        signature.copy_from_slice(&hex::decode("c49f65d9aad3b417f7d04a5e9c458b3308556bdff5a625bf65bfdadd11a18bb004bdb522991ae8648d6a1332a09576c90c93e6f9ea101bf8b5b3a7523958b50800").expect("hex decode"));
        let signature = Signature::from_slice(&signature[..]).unwrap();
        let tx = L2Transaction::new_builder()
            .raw(raw_tx)
            .signature(signature)
            .build();
        let eth = Secp256k1Eth {};

        // This rollup type hash is used, so the receiver script hash is:
        // 00002b003de527c1d67f2a2a348683ecc9598647c30884c89c5dcf6da1afbddd,
        // which contains leading zeros to ensure RLP behavior.
        let rollup_type_hash =
            hex::decode("cfdefce91f70f53167971f74bf1074b6b889be270306aabd34e67404b75dacab")
                .expect("hex decode");

        let mut sender_args = vec![];
        sender_args.extend(&rollup_type_hash);
        // Private key: dc88f509cab7f30ea36fd1aeb203403ce284e587bedecba73ba2fadf688acd19
        // Please do not use this private key elsewhere!
        sender_args
            .extend(&hex::decode("0000A7CE68e7328eCF2C83b103b50C68CF60Ae3a").expect("hex decode"));
        let sender_script = Script::new_builder()
            .args(Bytes::from(sender_args).pack())
            .build();

        let mut receiver_args = vec![];
        receiver_args.extend(&rollup_type_hash);
        receiver_args.extend(&23u32.to_le_bytes());
        let receiver_script = Script::new_builder()
            .args(Bytes::from(receiver_args).pack())
            .build();
        let ctx = RollupContext {
            rollup_script_hash: Default::default(),
            rollup_config: Default::default(),
        };
        let result = eth
            .verify_tx(&ctx, sender_script, receiver_script, tx)
            .expect("verify signature");
        assert!(result);
    }

    #[test]
    fn test_secp256k1_eth_polyjuice_create() {
        let mut polyjuice_args = vec![0u8; 69];
        polyjuice_args[0..7].copy_from_slice(b"\xFF\xFF\xFFPOLY");
        polyjuice_args[7] = 3;
        let gas_limit: u64 = 21000;
        polyjuice_args[8..16].copy_from_slice(&gas_limit.to_le_bytes());
        let gas_price: u128 = 20000000000;
        polyjuice_args[16..32].copy_from_slice(&gas_price.to_le_bytes());
        let value: u128 = 3000000;
        polyjuice_args[32..48].copy_from_slice(&value.to_le_bytes());
        let payload_length: u32 = 17;
        polyjuice_args[48..52].copy_from_slice(&payload_length.to_le_bytes());
        polyjuice_args[52..69].copy_from_slice(b"POLYJUICEcontract");

        let raw_tx = RawL2Transaction::new_builder()
            .nonce(9u32.pack())
            .to_id(23u32.pack())
            .args(Bytes::from(polyjuice_args).pack())
            .build();
        let mut signature = [0u8; 65];
        signature.copy_from_slice(&hex::decode("5289a4c910f143a97ce6d8ce55a970863c115bb95b404518a183ec470734ce0c10594e911d54d8894d05381fbc0f052b7397cd25217f6f102d297387a4cb15d700").expect("hex decode"));
        let signature = Signature::from_slice(&signature[..]).unwrap();
        let tx = L2Transaction::new_builder()
            .raw(raw_tx)
            .signature(signature)
            .build();
        let eth = Secp256k1Eth {};

        let rollup_type_hash = vec![0u8; 32];

        let mut sender_args = vec![];
        sender_args.extend(&rollup_type_hash);
        sender_args
            .extend(&hex::decode("9d8A62f656a8d1615C1294fd71e9CFb3E4855A4F").expect("hex decode"));
        let sender_script = Script::new_builder()
            .args(Bytes::from(sender_args).pack())
            .build();

        let mut receiver_args = vec![];
        receiver_args.extend(&rollup_type_hash);
        receiver_args.extend(&23u32.to_le_bytes());
        let receiver_script = Script::new_builder()
            .args(Bytes::from(receiver_args).pack())
            .build();
        let ctx = RollupContext {
            rollup_script_hash: Default::default(),
            rollup_config: Default::default(),
        };
        let result = eth
            .verify_tx(&ctx, sender_script, receiver_script, tx)
            .expect("verify signature");
        assert!(result);
    }

    #[test]
    fn test_secp256k1_eth_normal_call() {
        let raw_tx = RawL2Transaction::new_builder()
            .nonce(9u32.pack())
            .to_id(1234u32.pack())
            .build();
        let mut signature = [0u8; 65];
        signature.copy_from_slice(&hex::decode("680e9afc606f3555d75fedb41f201ade6a5f270c3a2223730e25d93e764acc6a49ee917f9e3af4727286ae4bf3ce19a5b15f71ae359cf8c0c3fabc212cccca1e00").expect("hex decode"));
        let signature = Signature::from_slice(&signature[..]).unwrap();
        let tx = L2Transaction::new_builder()
            .raw(raw_tx)
            .signature(signature)
            .build();
        let eth = Secp256k1Eth {};

        let rollup_type_hash = vec![0u8; 32];

        let mut sender_args = vec![];
        sender_args.extend(&rollup_type_hash);
        sender_args
            .extend(&hex::decode("9d8A62f656a8d1615C1294fd71e9CFb3E4855A4F").expect("hex decode"));
        let sender_script = Script::new_builder()
            .args(Bytes::from(sender_args).pack())
            .build();

        let mut receiver_args = vec![];
        receiver_args.extend(&rollup_type_hash);
        receiver_args.extend(&23u32.to_le_bytes());
        let receiver_script = Script::new_builder()
            .args(Bytes::from(receiver_args).pack())
            .build();
        let ctx = RollupContext {
            rollup_script_hash: Default::default(),
            rollup_config: Default::default(),
        };
        let result = eth
            .verify_tx(&ctx, sender_script, receiver_script, tx)
            .expect("verify signature");
        assert!(result);
    }

    #[test]
    fn test_secp256k1_tron() {
        let message = H256::from([0u8; 32]);
        let test_signature = Signature::from_slice(
        &hex::decode("702ec8cd52a61093519de11433595ee7177bc8beaef2836714efe23e01bbb45f7f4a51c079f16cc742a261fe53fa3d731704a7687054764d424bd92963a82a241b").expect("hex decode"))
        .expect("create signature structure");
        let address = Bytes::from(
            hex::decode("d0ebb370429e1cc8a7da1f7aeb2447083e15298b").expect("hex decode"),
        );
        let mut lock_args = vec![0u8; 32];
        lock_args.extend(address);
        let tron = Secp256k1Tron {};
        let result = tron
            .verify_withdrawal_signature(lock_args.into(), test_signature, message)
            .expect("verify signature");
        assert!(result);
    }
}
