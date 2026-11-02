//! A fund's own signing key. The first registration of a fund id binds its
//! public key; after that the registry accepts profile changes and rounds
//! only when this key signs them. Separate from the wallet: the note's
//! spending key proves control of the coins, this key proves which fund is
//! speaking.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use voting_circuits::ff::{Field, PrimeField};
use voting_circuits::rand::rngs::OsRng;
use voting_crypto_deps::orchard::primitives::redpallas::{Signature, SigningKey, SpendAuth, VerificationKey};
use voting_crypto_deps::pasta_curves::pallas;

use crate::util::b32;

/// Secret: stays with the fund, like its wallet.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FundKey {
    pub fund_id: String,
    pub signing_key: String,
}

impl FundKey {
    pub fn generate(fund_id: &str) -> Self {
        let s = pallas::Scalar::random(&mut OsRng);
        FundKey { fund_id: fund_id.into(), signing_key: hex::encode(s.to_repr()) }
    }

    fn sk(&self) -> Result<SigningKey<SpendAuth>> {
        SigningKey::try_from(b32(&self.signing_key)?).map_err(|_| anyhow!("bad fund signing key"))
    }

    /// The public key the registry binds to the fund id (hex).
    pub fn public(&self) -> Result<String> {
        let vk: [u8; 32] = (&VerificationKey::from(&self.sk()?)).into();
        Ok(hex::encode(vk))
    }

    pub fn sign(&self, msg: &str) -> Result<String> {
        let sig: [u8; 64] = (&self.sk()?.sign(OsRng, msg.as_bytes())).into();
        Ok(hex::encode(sig))
    }
}

/// Check that `key` is a valid fund public key (hex).
pub fn check_key(key: &str) -> Result<()> {
    VerificationKey::<SpendAuth>::try_from(b32(key)?).map(|_| ()).map_err(|_| anyhow!("not a valid fund key: {key}"))
}

/// Check `sig` over `msg` under the hex public key `key`.
pub fn verify_sig(key: &str, msg: &str, sig: &str) -> Result<()> {
    let vk = VerificationKey::<SpendAuth>::try_from(b32(key)?).map_err(|_| anyhow!("bad fund key"))?;
    let sig: [u8; 64] = hex::decode(sig)?.try_into().map_err(|_| anyhow!("bad signature length"))?;
    vk.verify(msg.as_bytes(), &Signature::from(sig)).map_err(|_| anyhow!("signature invalid"))
}
