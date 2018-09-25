use super::SECP;
use bitcoin::util::bip32::{self, ChildNumber, ExtendedPrivKey, ExtendedPubKey};
use crypto::{digest::Digest, sha2::Sha256};
use secp256k1_support::{KeyPair, SECRET_KEY_SIZE};
use std::{
    ops::DerefMut,
    sync::{Mutex, MutexGuard, PoisonError},
};
use uuid::Uuid;

#[derive(Debug)]
pub enum Error {
    Bip32Error(bip32::Error),
    IndexLockError,
}

impl From<bip32::Error> for Error {
    fn from(e: bip32::Error) -> Self {
        Error::Bip32Error(e)
    }
}

impl<'a> From<PoisonError<MutexGuard<'a, u32>>> for Error {
    fn from(_e: PoisonError<MutexGuard<'a, u32>>) -> Self {
        Error::IndexLockError
    }
}

pub struct KeyStore {
    _master_privkey: ExtendedPrivKey,
    transient_root_privkey: ExtendedPrivKey,
    internal_root_privkey: ExtendedPrivKey,
    next_internal_index: Mutex<u32>,
    // Do we want to remember already generated addresses or regenerate them?
    // Memory vs CPU -> could be a switch/option
    // Common practice for wallets is to pre-generate some addresses, hence:
    // TODO: manage a key pool
    // - key ready for use (pool)
    // - key already used
}

impl KeyStore {
    pub fn new(master_privkey: ExtendedPrivKey) -> Result<KeyStore, Error> {
        // As per bip32 and bitcoind reference implementation
        //
        // We use the following child keys:
        // m/0'/1 for bip32 internal chain (ie, where the BTC is sent after redeem, bitcoind-like)
        // m/0'/2' for HTLC (ie, locking the money in HTLC). 2 is an arbitrary value I chose
        // (1' being reserved for the bip32 external chain)
        // At this stage we expect an extended master private key in the configuration (m)
        // Then we just assume that we use account 0' (like bitcoind), hence we derive m/0'
        // and create our child keys from there.

        // TODO: set derivation path for Ethereum to m/44'/60'/a'/0/n, see #291
        // see https://github.com/ethereum/EIPs/issues/85 https://github.com/ethereum/EIPs/pull/600/files

        let account_0_privkey =
            master_privkey.ckd_priv(&*SECP, ChildNumber::from_hardened_idx(0))?;

        let internal_root_privkey =
            account_0_privkey.ckd_priv(&*SECP, ChildNumber::from_normal_idx(1))?;
        let transient_root_privkey =
            account_0_privkey.ckd_priv(&*SECP, ChildNumber::from_hardened_idx(2))?;

        Ok(KeyStore {
            _master_privkey: master_privkey,
            transient_root_privkey,
            internal_root_privkey,
            next_internal_index: Mutex::new(0),
            //transient_keys: HashMap::new(),
        })
    }

    pub fn get_new_internal_keypair(&self) -> Result<KeyPair, Error> {
        let priv_key = self.get_new_internal_privkey()?;
        let secret_key = priv_key.secret_key;

        Ok(KeyPair::from(secret_key))
    }

    pub fn get_new_internal_privkey(&self) -> Result<ExtendedPrivKey, Error> {
        let mut next_internal_index = self.next_internal_index.lock()?;
        let next_internal_index = next_internal_index.deref_mut();

        let res = self
            .internal_root_privkey
            .ckd_priv(&*SECP, ChildNumber::from_hardened_idx(*next_internal_index))?;

        // If we reach here, res is Ok
        *next_internal_index += 1;
        Ok(res)
    }

    pub fn get_internal_pubkey(&self, index: u32) -> Result<ExtendedPubKey, Error> {
        let priv_key = self
            .internal_root_privkey
            .ckd_priv(&*SECP, ChildNumber::from_hardened_idx(index))?;
        Ok(ExtendedPubKey::from_private(&*SECP, &priv_key))
    }

    fn new_secret_from_concat(data1: &[u8], data2: &[u8], data3: &[u8], secret: &mut [u8]) {
        let mut sha = Sha256::new();
        sha.input(data1);
        sha.input(data2);
        sha.input(data3);
        sha.result(secret);
    }

    pub fn get_transient_keypair(&self, uid: &Uuid, data: &[u8]) -> KeyPair {
        // SecretKey = SHA256(transient_root_privkey + id)
        let mut result: [u8; SECRET_KEY_SIZE] = [0; SECRET_KEY_SIZE];

        Self::new_secret_from_concat(
            &self.transient_root_privkey.secret_key[..],
            &uid.as_bytes()[..],
            &data[..],
            &mut result,
        );
        // This returns a result as it can fail if the slice is empty which is very unlikely hence the expect.
        KeyPair::from_secret_key_slice(&result).expect("Should never fail")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secp256k1_support::PublicKey;
    use std::str::FromStr;

    fn setup_keystore() -> KeyStore {
        let master_priv_key = ExtendedPrivKey::from_str(
        "xprv9s21ZrQH143K457pTbhs1LcmMnc4pCyqNTe9iEyoR8iTZeLtRzL6SpWCzK5iEP7fk72VhqkiNHuKQfqRVHTHBHQjxDDU7kTKHUuQCLNCbYi"
        ).unwrap();

        KeyStore::new(master_priv_key).unwrap()
    }

    #[test]
    fn internal_priv_and_pub_keys_sequential_generation() {
        let keystore = setup_keystore();

        let internal_privkey0 = keystore.get_new_internal_privkey().unwrap();
        let internal_privkey1 = keystore.get_new_internal_privkey().unwrap();
        let internal_privkey2 = keystore.get_new_internal_privkey().unwrap();

        assert_eq!(
            internal_privkey0.child_number,
            ChildNumber::from_hardened_idx(0)
        );
        assert_eq!(
            internal_privkey1.child_number,
            ChildNumber::from_hardened_idx(1)
        );
        assert_eq!(
            internal_privkey2.child_number,
            ChildNumber::from_hardened_idx(2)
        );
        assert_ne!(internal_privkey0, internal_privkey1);
        assert_ne!(internal_privkey1, internal_privkey2);
        assert_ne!(internal_privkey2, internal_privkey0);
    }

    #[test]
    fn internal_key_generation_child_numbers_are_correct() {
        let keystore = setup_keystore();

        let internal_pubkey0 = keystore.get_internal_pubkey(0).unwrap();
        let internal_pubkey1 = keystore.get_internal_pubkey(1).unwrap();
        let internal_pubkey2 = keystore.get_internal_pubkey(2).unwrap();

        assert_eq!(
            internal_pubkey0.child_number,
            ChildNumber::from_hardened_idx(0)
        );
        assert_eq!(
            internal_pubkey1.child_number,
            ChildNumber::from_hardened_idx(1)
        );
        assert_eq!(
            internal_pubkey2.child_number,
            ChildNumber::from_hardened_idx(2)
        );
        assert_ne!(internal_pubkey0, internal_pubkey1);
        assert_ne!(internal_pubkey1, internal_pubkey2);
        assert_ne!(internal_pubkey2, internal_pubkey0);
    }

    #[test]
    fn internal_key_generation_pub_keys_match() {
        let keystore = setup_keystore();

        let internal_privkey0 = keystore.get_new_internal_privkey().unwrap();
        let internal_privkey1 = keystore.get_new_internal_privkey().unwrap();
        let internal_privkey2 = keystore.get_new_internal_privkey().unwrap();
        let internal_pubkey0 = keystore.get_internal_pubkey(0).unwrap();
        let internal_pubkey1 = keystore.get_internal_pubkey(1).unwrap();
        let internal_pubkey2 = keystore.get_internal_pubkey(2).unwrap();

        let keypair_from_priv0 = KeyPair::from(internal_privkey0.secret_key);
        let keypair_from_priv1 = KeyPair::from(internal_privkey1.secret_key);
        let keypair_from_priv2 = KeyPair::from(internal_privkey2.secret_key);
        let pub_key_from_ext0 = internal_pubkey0.public_key;
        let pub_key_from_ext1 = internal_pubkey1.public_key;
        let pub_key_from_ext2 = internal_pubkey2.public_key;
        assert_eq!(
            keypair_from_priv0.public_key(),
            &PublicKey::from(pub_key_from_ext0)
        );
        assert_eq!(
            keypair_from_priv1.public_key(),
            &PublicKey::from(pub_key_from_ext1)
        );
        assert_eq!(
            keypair_from_priv2.public_key(),
            &PublicKey::from(pub_key_from_ext2)
        );
    }

    #[test]
    fn given_different_uid_same_data_generate_diff_transient_keys() {
        let master_priv_key = ExtendedPrivKey::from_str(
            "xprv9s21ZrQH143K457pTbhs1LcmMnc4pCyqNTe9iEyoR8iTZeLtRzL6SpWCzK5iEP7fk72VhqkiNHuKQfqRVHTHBHQjxDDU7kTKHUuQCLNCbYi"
        ).unwrap();

        let keystore = KeyStore::new(master_priv_key).unwrap();

        let uid0 = Uuid::new_v4();
        let uid1 = Uuid::new_v4();
        let data = vec![1u8];

        let transient_keypair0 = keystore.get_transient_keypair(&uid0, &data);
        let transient_keypair1 = keystore.get_transient_keypair(&uid1, &data);

        assert_ne!(transient_keypair0, transient_keypair1);
    }

    #[test]
    fn given_same_uid_different_data_generate_diff_transient_keys() {
        let master_priv_key = ExtendedPrivKey::from_str(
            "xprv9s21ZrQH143K457pTbhs1LcmMnc4pCyqNTe9iEyoR8iTZeLtRzL6SpWCzK5iEP7fk72VhqkiNHuKQfqRVHTHBHQjxDDU7kTKHUuQCLNCbYi"
        ).unwrap();

        let keystore = KeyStore::new(master_priv_key).unwrap();

        let uid = Uuid::new_v4();
        let data0 = vec![0u8];
        let data1 = vec![1u8];

        let transient_keypair0 = keystore.get_transient_keypair(&uid, &data0);
        let transient_keypair1 = keystore.get_transient_keypair(&uid, &data1);

        assert_ne!(transient_keypair0, transient_keypair1);
    }
}
