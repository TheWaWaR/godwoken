use gw_common::H256;
use gw_types::{bytes::Bytes, packed::Script};

pub trait CodeStore {
    fn insert_script(&mut self, script_hash: H256, script: Script);
    fn get_script(&self, script_hash: &H256) -> Option<Script>;
    fn insert_data(&mut self, data_hash: H256, code: Bytes);
    fn get_data(&self, data_hash: &H256) -> Option<Bytes>;
}

pub trait ChainStore {
    fn get_block_hash_by_number(&self, number: u64) -> Result<Option<H256>, gw_db::error::Error>;
    fn get_block_number_by_hash(
        &self,
        block_hash: &H256,
    ) -> Result<Option<u64>, gw_db::error::Error>;
}
