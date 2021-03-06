use crate::swap_protocols::{
    self,
    rfc003::secret::{Secret, SecretHash},
};
use serde::{de::DeserializeOwned, Serialize};
use std::{fmt::Debug, hash::Hash};

pub trait Ledger: swap_protocols::Ledger {
    type LockDuration: PartialEq
        + Eq
        + Hash
        + Debug
        + Clone
        + Send
        + Sync
        + Serialize
        + DeserializeOwned
        + 'static;
    type HtlcLocation: PartialEq + Debug + Clone + DeserializeOwned + Serialize + Send + Sync;
    type HtlcIdentity: Clone
        + Send
        + Sync
        + PartialEq
        + Debug
        + Into<<Self as swap_protocols::ledger::Ledger>::Identity>;
}

pub trait ExtractSecret {
    fn extract_secret(&self, secret_hash: &SecretHash) -> Option<Secret>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FundTransaction<L: Ledger>(pub L::Transaction);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RedeemTransaction<L: Ledger> {
    pub transaction: L::Transaction,
    pub secret: Secret,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefundTransaction<L: Ledger>(pub L::Transaction);

impl<L: Ledger> AsRef<L::Transaction> for FundTransaction<L> {
    fn as_ref(&self) -> &L::Transaction {
        &self.0
    }
}

impl<L: Ledger> AsRef<L::Transaction> for RefundTransaction<L> {
    fn as_ref(&self) -> &L::Transaction {
        &self.0
    }
}
