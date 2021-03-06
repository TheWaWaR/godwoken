use gw_common::H256;
use gw_types::packed::{ChallengeTarget, ChallengeWitness, RollupConfig};
use std::fmt::{self, Display};

#[derive(Clone)]
pub struct RollupContext {
    pub rollup_script_hash: H256,
    pub rollup_config: RollupConfig,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ChallengeContext {
    pub target: ChallengeTarget,
    pub witness: ChallengeWitness,
}

impl Display for ChallengeContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{target: {}, witness: {}}}", self.target, self.witness)
    }
}
