#[macro_use]
mod transition_save;

pub mod alice;
pub mod bitcoin;
pub mod bob;
pub mod ethereum;
pub mod events;
pub mod find_htlc_location;

pub mod state_machine;
pub mod state_store;

mod error;

mod ledger;
mod role;
mod save_state;
mod secret;
mod secret_source;

#[cfg(test)]
mod state_machine_test;

pub use self::{
    alice::Alice,
    bob::Bob,
    error::Error,
    ledger::{ExtractSecret, FundTransaction, Ledger, RedeemTransaction, RefundTransaction},
    role::*,
    save_state::SaveState,
    secret::{RandomnessSource, Secret, SecretFromErr, SecretHash},
    secret_source::*,
};
