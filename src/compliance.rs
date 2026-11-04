//! Certificates: a signed statement, by someone other than the company, that
//! a round's reserve proof verifies.
//!
//! Two kinds of issuer:
//! - the registry, which certifies every round it accepts, under its own key;
//! - an authority (auditor, regulator, compliance body) the registry operator
//!   has approved. A company asks it to certify a round; the authority
//!   re-verifies the proof on its own machine against its own chain data and
//!   signs a certificate with its own key, or declines.
//!
//! Certificates are private to the company: they never appear on the fund's
//! link page. The company reads them with a request signed by its fund key and
//! hands them to whoever needs them. Anyone can check one with
//! `zsolv certificate verify`.

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use voting_circuits::ff::{Field, PrimeField};
use voting_circuits::rand::rngs::OsRng;
use voting_crypto_deps::orchard::primitives::redpallas::{SigningKey, SpendAuth, VerificationKey};
use voting_crypto_deps::pasta_curves::pallas;

use crate::attest::{Attestation, Verdict, VERSION};
use crate::fundkey::verify_sig;
use crate::util::{b32, short, zec};

pub const ROLES: [&str; 3] = ["auditor", "regulator", "compliance"];

/// How long a signed request stays fresh, in seconds.
pub const FRESH_SECS: u64 = 600;

/// A signing key for an issuer: an authority, or the registry itself. Secret.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IssuerKey {
    pub name: String,
    /// auditor, regulator or compliance; "registry" for the registry's own key.
    pub role: String,
    pub signing_key: String,
}

impl IssuerKey {
    pub fn generate(name: &str, role: &str) -> Self {
        let s = pallas::Scalar::random(&mut OsRng);
        IssuerKey { name: name.into(), role: role.into(), signing_key: hex::encode(s.to_repr()) }
    }
    fn sk(&self) -> Result<SigningKey<SpendAuth>> {
        SigningKey::try_from(b32(&self.signing_key)?).map_err(|_| anyhow!("bad issuer signing key"))
    }
    pub fn public(&self) -> Result<String> {
        let vk: [u8; 32] = (&VerificationKey::from(&self.sk()?)).into();
        Ok(hex::encode(vk))
    }
    pub fn sign(&self, msg: &str) -> Result<String> {
        let sig: [u8; 64] = (&self.sk()?.sign(OsRng, msg.as_bytes())).into();
        Ok(hex::encode(sig))
    }
}

pub fn check_role(role: &str) -> Result<()> {
    if !ROLES.contains(&role) {
        bail!("role must be one of {}", ROLES.join(", "));
    }
    Ok(())
}

pub fn check_name(name: &str) -> Result<()> {
    if name.trim().is_empty() || name.len() > 120 {
        bail!("give the authority a name of up to 120 characters");
    }
    Ok(())
}

/// An authority the registry operator approved to issue certificates.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Authority {
    pub key: String,
    pub name: String,
    pub role: String,
    pub approved_at: String,
}

/// A signed certificate for one round of one fund.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Certificate {
    pub version: String,
    /// Short id: the start of the hash of the signed message.
    pub id: String,
    /// "registry" or "authority".
    pub kind: String,
    pub registry: String,
    pub issuer_name: String,
    pub issuer_role: String,
    pub issuer_key: String,
    pub fund_id: String,
    pub fund_name: String,
    pub fund_key: String,
    pub round: u64,
    pub threshold_zat: u64,
    pub since_height: u64,
    pub at_height: u64,
    pub at_block_hash: String,
    /// Hash of the round message the fund signed: ties the certificate to that exact statement.
    pub statement_hash: String,
    pub checks_passed: usize,
    pub checks_total: usize,
    #[serde(default)]
    pub note: String,
    /// Unix seconds.
    pub issued_at: String,
    pub signature: String,
}

pub fn hash_hex(s: &str) -> String {
    hex::encode(blake2b_simd::Params::new().hash_length(32).personal(b"ZSolvency_Cert__").hash(s.as_bytes()).as_bytes())
}

/// The text the issuer signs: every field but the id and the signature.
pub fn certificate_message(c: &Certificate) -> String {
    format!(
        "{VERSION} certificate\nkind: {}\nregistry: {}\nissuer: {}\nissuer_role: {}\nissuer_key: {}\nfund: {}\nfund_name: {}\nfund_key: {}\nround: {}\nthreshold_zat: {}\nsince_height: {}\nat_height: {}\nat_block_hash: {}\nstatement_hash: {}\nchecks: {}/{}\nnote: {}\nissued_at: {}\n",
        c.kind, c.registry, c.issuer_name, c.issuer_role, c.issuer_key, c.fund_id, c.fund_name, c.fund_key, c.round,
        c.threshold_zat, c.since_height, c.at_height, c.at_block_hash, c.statement_hash, c.checks_passed, c.checks_total,
        c.note, c.issued_at
    )
}

/// Build and sign a certificate for an attestation the issuer has verified.
pub fn issue(kind: &str, issuer: &IssuerKey, a: &Attestation, v: &Verdict, fund_name: &str, note: &str) -> Result<Certificate> {
    if !v.ok {
        bail!("the proof doesn't verify, so it can't be certified");
    }
    if note.len() > 500 {
        bail!("keep the note under 500 characters");
    }
    let mut c = Certificate {
        version: format!("{VERSION} certificate"),
        id: String::new(),
        kind: kind.into(),
        registry: a.registry.clone(),
        issuer_name: issuer.name.clone(),
        issuer_role: issuer.role.clone(),
        issuer_key: issuer.public()?,
        fund_id: a.fund_id.clone(),
        fund_name: fund_name.into(),
        fund_key: a.fund_key.clone(),
        round: a.round,
        threshold_zat: a.threshold_zat,
        since_height: a.since_height,
        at_height: a.at_height,
        at_block_hash: a.at_block_hash.clone(),
        statement_hash: hash_hex(&a.message),
        checks_passed: v.checks.iter().filter(|c| c.ok).count(),
        checks_total: v.checks.len(),
        note: note.trim().into(),
        issued_at: crate::registry::now(),
        signature: String::new(),
    };
    let msg = certificate_message(&c);
    c.id = hash_hex(&msg)[..16].to_string();
    c.signature = issuer.sign(&msg)?;
    Ok(c)
}

/// Check a certificate's id and signature. Says nothing about whether its
/// issuer is trusted: compare `issuer_key` with the registry's key or its
/// approved authorities for that.
pub fn check_certificate(c: &Certificate) -> Result<()> {
    let msg = certificate_message(c);
    if c.id != hash_hex(&msg)[..16] {
        bail!("certificate fields were changed after it was issued");
    }
    verify_sig(&c.issuer_key, &msg, &c.signature).map_err(|_| anyhow!("certificate signature invalid: not signed by {}", short(&c.issuer_key)))
}

/// Does the certificate describe exactly this attestation?
pub fn matches(c: &Certificate, a: &Attestation) -> bool {
    c.registry == a.registry
        && c.fund_id == a.fund_id
        && c.fund_key == a.fund_key
        && c.round == a.round
        && c.threshold_zat == a.threshold_zat
        && c.since_height == a.since_height
        && c.at_height == a.at_height
        && c.at_block_hash == a.at_block_hash
        && c.statement_hash == hash_hex(&a.message)
}

/// One line for people: what the certificate says.
pub fn summary(c: &Certificate, unit: &str) -> String {
    format!(
        "{} ({}) certifies that {} held at least {} {unit} from block {} to block {}, round {}",
        c.issuer_name, c.issuer_role, c.fund_name, zec(c.threshold_zat), c.since_height, c.at_height, c.round
    )
}

/// A request for certification, as stored by the registry.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CertRequest {
    pub id: String,
    pub fund_id: String,
    pub authority_key: String,
    pub round: u64,
    #[serde(default)]
    pub note: String,
    pub requested_at: String,
    /// pending, issued or declined.
    pub status: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub decided_at: Option<String>,
    #[serde(default)]
    pub certificate: Option<Certificate>,
}

// ------------------------------------------------------------ signed requests

/// A request signed by a fund or authority key, fresh within `FRESH_SECS`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Signed {
    pub key: String,
    pub time: u64,
    pub signature: String,
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn check_fresh(time: u64) -> Result<()> {
    let now = unix_now();
    if time.abs_diff(now) > FRESH_SECS {
        bail!("request is too old or from the future: check this computer's clock");
    }
    Ok(())
}

pub fn fund_read_message(registry: &str, fund_id: &str, key: &str, time: u64) -> String {
    format!("{VERSION} certificates\nregistry: {registry}\nfund: {fund_id}\nfund_key: {key}\ntime: {time}\n")
}

pub fn request_message(registry: &str, fund_id: &str, authority: &str, round: u64, note: &str, time: u64) -> String {
    format!("{VERSION} certification request\nregistry: {registry}\nfund: {fund_id}\nauthority: {authority}\nround: {round}\nnote: {note}\ntime: {time}\n")
}

pub fn inbox_message(registry: &str, authority: &str, time: u64) -> String {
    format!("{VERSION} authority inbox\nregistry: {registry}\nauthority: {authority}\ntime: {time}\n")
}

pub fn decline_message(registry: &str, request: &str, authority: &str, reason: &str, time: u64) -> String {
    format!("{VERSION} certification declined\nregistry: {registry}\nrequest: {request}\nauthority: {authority}\nreason: {reason}\ntime: {time}\n")
}

/// Body of `POST /api/certificates/request`, signed by the fund key.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RequestBody {
    pub fund_id: String,
    pub authority_key: String,
    pub round: u64,
    #[serde(default)]
    pub note: String,
    pub signed: Signed,
}

/// Body of `POST /api/certificates/fund`, signed by the fund key.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FundReadBody {
    pub fund_id: String,
    pub signed: Signed,
}

/// Body of `POST /api/certificates/decline`, signed by the authority key.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeclineBody {
    pub request_id: String,
    pub reason: String,
    pub signed: Signed,
}
