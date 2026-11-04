//! The public registry: stores fund profiles and attestations, verifies each
//! attestation against chain data, and refuses a registry nullifier that is
//! already counted by another fund. Serves the reserve pages.
//!
//! Funds are unlisted: nothing enumerates them. Each fund gets a random link
//! token at first registration, returned only to whoever signs as the fund,
//! and its page, attestation and Verify are reachable only through that link.
//!
//! The first registration of a fund id binds the fund's public key. After
//! that, profile changes and rounds are accepted only when that key signs
//! them; anything else is refused without touching the fund's page. An
//! operator can close registration to listed (id, key) pairs.
//!
//! Requests that take seconds (proof checks, root rebuilds) run one at a
//! time on a worker thread, so the pages keep loading meanwhile.

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::attest::{round_message, verify, Attestation, Verdict, VERSION};
use crate::compliance::{
    check_certificate, check_fresh, check_name, check_role, decline_message, fund_read_message, inbox_message, issue, matches,
    request_message, Authority, CertRequest, Certificate, DeclineBody, FundReadBody, IssuerKey, RequestBody, Signed,
};
use crate::fundkey::{check_key, verify_sig, FundKey};
use crate::ledger::Ledger;
use crate::store::{read_json, write_json, DataDir};
use crate::util::{short, zec, MAX_MONEY};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Obligation {
    pub label: String,
    pub zat: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StoredAttestation {
    pub received_at: String,
    pub attestation: Attestation,
    pub verdict: Verdict,
    /// The registry's own certificate for this round. Private to the fund.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry_certificate: Option<Certificate>,
}

/// What a fund publishes about itself. Signed by the fund key on every change.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Profile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub obligations: Vec<Obligation>,
    /// Blocks the reserve must have been held to count as "held", e.g. 4 weeks.
    #[serde(default)]
    pub min_held_blocks: u64,
    /// The fund promises a new proof at least this often; an older latest
    /// proof shows amber. 0 turns the check off.
    #[serde(default)]
    pub max_proof_age_blocks: u64,
    #[serde(default)]
    pub zec_usd: Option<f64>,
    #[serde(default)]
    pub price_time: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Fund {
    #[serde(flatten)]
    pub profile: Profile,
    /// The fund's public key, bound at first registration.
    pub key: String,
    /// Sequence number of the last accepted profile update.
    pub profile_seq: u64,
    #[serde(default)]
    pub attestations: Vec<StoredAttestation>,
    /// The unguessable token in the fund's share link. Assigned at first
    /// registration; the fund shares it, the registry never lists it.
    #[serde(default)]
    pub link: String,
}

/// A fresh link token: 128 random bits, hex.
pub fn new_link() -> String {
    use voting_circuits::rand::{rngs::OsRng, Rng};
    let mut b = [0u8; 16];
    OsRng.fill_bytes(&mut b);
    hex::encode(b)
}

/// A profile change as sent to `POST /api/funds`. `profile` is the JSON text
/// that was signed, so the registry checks exactly the bytes the fund signed.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProfileUpdate {
    pub profile: String,
    pub key: String,
    pub seq: u64,
    pub signature: String,
}

pub fn profile_message(registry: &str, fund_id: &str, key: &str, seq: u64, profile: &str) -> String {
    format!("{VERSION} profile\nregistry: {registry}\nfund: {fund_id}\nfund_key: {key}\nseq: {seq}\nprofile: {profile}\n")
}

/// Sign a profile change for `registry`. `seq` must exceed the last accepted one.
pub fn sign_profile(registry: &str, p: &Profile, key: &FundKey, seq: u64) -> Result<ProfileUpdate> {
    if key.fund_id != p.id {
        bail!("the fund key is for {}, not {}", key.fund_id, p.id);
    }
    let profile = serde_json::to_string(p)?;
    let pk = key.public()?;
    let signature = key.sign(&profile_message(registry, &p.id, &pk, seq, &profile))?;
    Ok(ProfileUpdate { profile, key: pk, seq, signature })
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Event {
    pub time: String,
    pub fund_id: String,
    pub accepted: bool,
    pub summary: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Registry {
    pub id: String,
    pub funds: BTreeMap<String, Fund>,
    /// registry nullifier → fund id that first counted it.
    pub nullifier_owner: BTreeMap<String, String>,
    pub events: Vec<Event>,
    /// Closed registration: new fund ids only with these keys (id → public
    /// key). None: anyone may register an unused id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowlist: Option<BTreeMap<String, String>>,
    /// Auditors, regulators and compliance bodies the operator approved to
    /// issue certificates, by public key.
    #[serde(default)]
    pub authorities: BTreeMap<String, Authority>,
    /// Requests for certification, from funds to authorities.
    #[serde(default)]
    pub cert_requests: Vec<CertRequest>,
}

/// Body of `POST /api/certificates/issue`: a certificate signed by an approved authority.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IssueBody {
    pub request_id: String,
    pub certificate: Certificate,
}

/// Most open requests a fund may have at once, so nobody fills an authority's inbox.
const MAX_PENDING_PER_FUND: usize = 20;

const MAX_ID: usize = 64;
const MAX_NAME: usize = 120;
const MAX_TEXT: usize = 2000;
const MAX_LABEL: usize = 200;
const MAX_OBLIGATIONS: usize = 50;
/// The event log keeps the most recent entries only.
pub const MAX_EVENTS: usize = 2000;

/// What a profile may say: keeps the page readable and the totals in range.
pub fn check_profile(p: &Profile) -> Result<()> {
    if p.id.is_empty() || p.id.len() > MAX_ID || !p.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        bail!("fund id must be 1 to {MAX_ID} letters, digits and dashes");
    }
    let n = |s: &str| s.chars().count();
    if p.name.trim().is_empty() || n(&p.name) > MAX_NAME {
        bail!("name must be 1 to {MAX_NAME} characters");
    }
    if n(&p.description) > MAX_TEXT {
        bail!("description must be at most {MAX_TEXT} characters");
    }
    if p.obligations.len() > MAX_OBLIGATIONS {
        bail!("at most {MAX_OBLIGATIONS} obligations");
    }
    let mut total = 0u64;
    for o in &p.obligations {
        if n(&o.label) > MAX_LABEL {
            bail!("obligation labels must be at most {MAX_LABEL} characters");
        }
        total = total.checked_add(o.zat).filter(|t| *t <= MAX_MONEY).ok_or_else(|| anyhow!("obligations add up to more than 21 million ZEC"))?;
    }
    if p.price_time.as_deref().is_some_and(|t| n(t) > MAX_NAME) {
        bail!("price_time must be at most {MAX_NAME} characters");
    }
    if p.zec_usd.is_some_and(|x| !(x.is_finite() && x >= 0.0)) {
        bail!("zec_usd must be a non-negative number");
    }
    Ok(())
}

/// Chain time at Zcash's 75-second block target.
pub const HOUR_BLOCKS: u64 = 48;
pub const DAY_BLOCKS: u64 = 1152;
pub const WEEK_BLOCKS: u64 = 8064;

/// A stretch of chain time in words: blocks under an hour, then hours, then days.
pub fn span(blocks: u64) -> String {
    let p = |n: u64, w: &str| format!("{n} {w}{}", if n == 1 { "" } else { "s" });
    if blocks < HOUR_BLOCKS {
        p(blocks, "block")
    } else if blocks < 2 * DAY_BLOCKS {
        p(blocks / HOUR_BLOCKS, "hour")
    } else {
        p(blocks / DAY_BLOCKS, "day")
    }
}

fn cadence_words(c: u64) -> String {
    match c {
        WEEK_BLOCKS => "weekly".into(),
        DAY_BLOCKS => "daily".into(),
        _ => format!("every {}", span(c)),
    }
}

/// The light a round earns on its own (status rules 6 to 8): T against the
/// obligations, and how long the reserve was held against the promise.
fn round_light(p: &Profile, a: &Attestation, obligations_zat: u64, unit: &str) -> (&'static str, String) {
    let held = a.at_height.saturating_sub(a.since_height);
    if a.threshold_zat < obligations_zat {
        ("amber", format!("Reserve {} {unit} is below obligations of {} {unit}", zec(a.threshold_zat), zec(obligations_zat)))
    } else if held < p.min_held_blocks {
        ("amber", format!("Reserve held {} (since block {}); the fund promises {}", span(held), a.since_height, span(p.min_held_blocks)))
    } else {
        ("green", format!("Reserves cover obligations, held since block {}", a.since_height))
    }
}

impl Registry {
    pub fn new(id: &str) -> Self {
        Registry {
            id: id.into(),
            funds: BTreeMap::new(),
            nullifier_owner: BTreeMap::new(),
            events: vec![],
            allowlist: None,
            authorities: BTreeMap::new(),
            cert_requests: vec![],
        }
    }
    /// The registry state in `d`, which must be for registry `id`.
    pub fn load_or_new(d: &DataDir, id: &str) -> Result<Self> {
        if !d.registry().exists() {
            return Ok(Self::new(id));
        }
        let r: Self = read_json(&d.registry())?;
        if r.id != id {
            bail!("{} holds registry {}, not {id} (pass --registry {} or use another data directory)", d.registry().display(), r.id, r.id);
        }
        Ok(r)
    }
    pub fn save(&self, d: &DataDir) -> Result<()> {
        write_json(&d.registry(), self)
    }

    /// Register a fund, or change its profile. A new id binds `u.key`; an
    /// existing one needs a signature by its bound key and a newer `seq`.
    pub fn register(&mut self, u: ProfileUpdate) -> Result<String> {
        let p: Profile = serde_json::from_str(&u.profile).map_err(|e| anyhow!("bad profile: {e}"))?;
        check_profile(&p)?;
        verify_sig(&u.key, &profile_message(&self.id, &p.id, &u.key, u.seq, &u.profile), &u.signature)
            .map_err(|_| anyhow!("profile signature invalid (signed for another registry? this is {})", self.id))?;
        let id = p.id.clone();
        match self.funds.get_mut(&id) {
            Some(f) => {
                if f.key != u.key {
                    bail!("fund {id} is registered to another key; only that key can change its profile");
                }
                if u.seq <= f.profile_seq {
                    bail!("profile update {} is not newer than {} already accepted", u.seq, f.profile_seq);
                }
                f.profile = p;
                f.profile_seq = u.seq;
            }
            None => {
                if let Some(allow) = &self.allowlist {
                    if allow.get(&id) != Some(&u.key) {
                        bail!("registration on this registry is by invitation: ask its operator to run `zsolv registry allow --id {id} --key {}`", u.key);
                    }
                }
                self.funds.insert(id.clone(), Fund { profile: p, key: u.key, profile_seq: u.seq, attestations: vec![], link: new_link() });
            }
        }
        Ok(id)
    }

    /// Give a link to funds registered before links existed. True if any changed.
    pub fn ensure_links(&mut self) -> bool {
        let mut changed = false;
        for f in self.funds.values_mut().filter(|f| f.link.is_empty()) {
            f.link = new_link();
            changed = true;
        }
        changed
    }

    /// The fund a share link points to.
    pub fn by_link(&self, link: &str) -> Option<&Fund> {
        if link.is_empty() {
            return None;
        }
        self.funds.values().find(|f| f.link == link)
    }

    fn event(&mut self, fund_id: &str, accepted: bool, summary: String) {
        self.events.push(Event { time: now(), fund_id: fund_id.into(), accepted, summary });
        if self.events.len() > MAX_EVENTS {
            let extra = self.events.len() - MAX_EVENTS;
            self.events.drain(..extra);
        }
    }

    /// Verify and record an attestation. Returns (accepted, reason, verdict).
    pub fn submit(&mut self, a: Attestation, ledger: &Ledger) -> Result<(bool, String, Option<Verdict>)> {
        let fund_id = a.fund_id.clone();
        let Some(fund) = self.funds.get(&fund_id) else {
            bail!("unknown fund {fund_id}: register its profile first");
        };
        // Refused without an event: only the fund's own fresh, signed rounds
        // may write to its page, so nobody else can fill it with refusals.
        if a.fund_key != fund.key {
            return Ok((false, format!("not signed by the key registered for {fund_id}"), None));
        }
        if a.message != round_message(&a) || verify_sig(&fund.key, &a.message, &a.fund_signature).is_err() {
            return Ok((false, format!("fund signature invalid for {fund_id}"), None));
        }
        if let Some(last) = fund.attestations.last() {
            if a.round <= last.attestation.round {
                return Ok((false, format!("round {} is not newer than round {} already published", a.round, last.attestation.round), None));
            }
            if a.at_height < last.attestation.at_height {
                return Ok((false, format!("round {} is checked at block {}, before round {}'s block {}", a.round, a.at_height, last.attestation.round, last.attestation.at_height), None));
            }
        }
        if a.registry != self.id {
            let r = format!("attestation is for registry {}, this is {}", a.registry, self.id);
            self.event(&fund_id, false, r.clone());
            return Ok((false, r, None));
        }
        let v = verify(&a, ledger);
        if !v.ok {
            let failed: Vec<_> = v.checks.iter().filter(|c| !c.ok).map(|c| c.detail.clone()).collect();
            let r = format!("verification failed: {}", failed.join("; "));
            self.event(&fund_id, false, r.clone());
            return Ok((false, r, Some(v)));
        }
        for n in a.registry_nullifiers() {
            if let Some(owner) = self.nullifier_owner.get(n) {
                if owner != &fund_id {
                    // Funds are unlisted, so the refusal doesn't say which fund.
                    let r = format!("note already counted by another fund (registry nullifier {})", short(n));
                    self.event(&fund_id, false, r.clone());
                    return Ok((false, r, Some(v)));
                }
            }
        }
        for n in a.registry_nullifiers() {
            self.nullifier_owner.insert(n.clone(), fund_id.clone());
        }
        let summary = format!(
            "round {}: at least {} {}, held since block {}, checked at block {}",
            a.round, zec(a.threshold_zat), ledger.unit(), a.since_height, a.at_height
        );
        self.event(&fund_id, true, summary.clone());
        self.funds.get_mut(&fund_id).unwrap().attestations.push(StoredAttestation { received_at: now(), attestation: a, verdict: v.clone(), registry_certificate: None });
        Ok((true, summary, Some(v)))
    }

    // -------------------------------------------------------------- certificates

    /// Approve an authority to issue certificates (operator only, from the CLI).
    pub fn approve_authority(&mut self, key: &str, name: &str, role: &str) -> Result<()> {
        check_key(key)?;
        check_name(name)?;
        check_role(role)?;
        self.authorities.insert(key.into(), Authority { key: key.into(), name: name.trim().into(), role: role.into(), approved_at: now() });
        Ok(())
    }

    pub fn revoke_authority(&mut self, key: &str) -> Result<Authority> {
        self.authorities.remove(key).ok_or_else(|| anyhow!("no approved authority with key {}", short(key)))
    }

    /// The registry's own certificate for a fund's latest round, issued once.
    pub fn certify_latest(&mut self, fund_id: &str, key: &IssuerKey) -> Result<Certificate> {
        let f = self.funds.get_mut(fund_id).ok_or_else(|| anyhow!("unknown fund {fund_id}"))?;
        let name = f.profile.name.clone();
        let s = f.attestations.last_mut().ok_or_else(|| anyhow!("{fund_id} has no rounds"))?;
        if let Some(c) = &s.registry_certificate {
            return Ok(c.clone());
        }
        let c = issue("registry", key, &s.attestation, &s.verdict, &name, "Verified by the registry when the round was published.")?;
        s.registry_certificate = Some(c.clone());
        Ok(c)
    }

    fn fund_signed(&self, fund_id: &str, s: &Signed, msg: &str) -> Result<&Fund> {
        let f = self.funds.get(fund_id).ok_or_else(|| anyhow!("unknown fund {fund_id}"))?;
        if s.key != f.key {
            bail!("not signed by the key registered for {fund_id}");
        }
        check_fresh(s.time)?;
        verify_sig(&f.key, msg, &s.signature).map_err(|_| anyhow!("signature invalid for {fund_id}"))?;
        Ok(f)
    }

    fn authority_signed(&self, s: &Signed, msg: &str) -> Result<&Authority> {
        let a = self.authorities.get(&s.key).ok_or_else(|| {
            anyhow!("this authority isn't approved on registry {}: ask its operator to run `zsolv registry approve-authority --key {}`", self.id, s.key)
        })?;
        check_fresh(s.time)?;
        verify_sig(&s.key, msg, &s.signature).map_err(|_| anyhow!("signature invalid for this authority"))?;
        Ok(a)
    }

    /// A fund asks an approved authority to certify one of its published rounds.
    pub fn request_certification(&mut self, b: RequestBody) -> Result<CertRequest> {
        let msg = request_message(&self.id, &b.fund_id, &b.authority_key, b.round, &b.note, b.signed.time);
        let f = self.fund_signed(&b.fund_id, &b.signed, &msg)?;
        if !f.attestations.iter().any(|s| s.attestation.round == b.round) {
            bail!("round {} isn't published on this registry", b.round);
        }
        if !self.authorities.contains_key(&b.authority_key) {
            bail!("that authority isn't approved on this registry");
        }
        if b.note.len() > 500 {
            bail!("keep the note under 500 characters");
        }
        let open: Vec<_> = self.cert_requests.iter().filter(|r| r.fund_id == b.fund_id && r.status == "pending").collect();
        if open.iter().any(|r| r.authority_key == b.authority_key && r.round == b.round) {
            bail!("round {} is already waiting for this authority", b.round);
        }
        if open.len() >= MAX_PENDING_PER_FUND {
            bail!("{} requests are already open; wait for answers first", open.len());
        }
        let r = CertRequest {
            id: new_link()[..16].to_string(),
            fund_id: b.fund_id,
            authority_key: b.authority_key,
            round: b.round,
            note: b.note.trim().into(),
            requested_at: now(),
            status: "pending".into(),
            reason: None,
            decided_at: None,
            certificate: None,
        };
        self.cert_requests.push(r.clone());
        Ok(r)
    }

    /// Everything certified for a fund, for the fund only.
    pub fn fund_certificates(&self, b: &FundReadBody) -> Result<serde_json::Value> {
        let f = self.fund_signed(&b.fund_id, &b.signed, &fund_read_message(&self.id, &b.fund_id, &b.signed.key, b.signed.time))?;
        let registry_certificates: Vec<_> = f.attestations.iter().rev().filter_map(|s| s.registry_certificate.clone()).collect();
        let requests: Vec<_> = self
            .cert_requests
            .iter()
            .rev()
            .filter(|r| r.fund_id == b.fund_id)
            .map(|r| {
                let a = self.authorities.get(&r.authority_key);
                serde_json::json!({ "request": r, "authority_name": a.map(|a| a.name.clone()), "authority_role": a.map(|a| a.role.clone()) })
            })
            .collect();
        Ok(serde_json::json!({ "registry_certificates": registry_certificates, "requests": requests }))
    }

    /// An authority's requests, newest first, with what it needs to check each one.
    pub fn authority_inbox(&self, s: &Signed) -> Result<serde_json::Value> {
        let a = self.authority_signed(s, &inbox_message(&self.id, &s.key, s.time))?;
        let items: Vec<_> = self
            .cert_requests
            .iter()
            .rev()
            .filter(|r| r.authority_key == s.key)
            .map(|r| {
                let f = self.funds.get(&r.fund_id);
                let st = f.and_then(|f| f.attestations.iter().find(|x| x.attestation.round == r.round));
                serde_json::json!({
                    "request": r,
                    "fund_name": f.map(|f| f.profile.name.clone()),
                    "fund_description": f.map(|f| f.profile.description.clone()),
                    "obligations_zat": f.map(|f| f.profile.obligations.iter().fold(0u64, |t, o| t.saturating_add(o.zat))),
                    "link": f.map(|f| f.link.clone()),
                    "attestation": st.map(|x| &x.attestation),
                })
            })
            .collect();
        Ok(serde_json::json!({ "authority": a, "requests": items }))
    }

    /// Record a certificate an approved authority signed for a pending request.
    pub fn issue_certificate(&mut self, b: IssueBody) -> Result<CertRequest> {
        let c = &b.certificate;
        let i = self.cert_requests.iter().position(|r| r.id == b.request_id).ok_or_else(|| anyhow!("no request {}", b.request_id))?;
        let r = &self.cert_requests[i];
        if r.status != "pending" {
            bail!("request {} is already {}", r.id, r.status);
        }
        let auth = self.authorities.get(&r.authority_key).ok_or_else(|| anyhow!("that authority is no longer approved"))?;
        if c.kind != "authority" || c.issuer_key != r.authority_key || c.issuer_name != auth.name || c.issuer_role != auth.role {
            bail!("the certificate isn't from the authority this request was sent to");
        }
        check_certificate(c)?;
        let f = self.funds.get(&r.fund_id).ok_or_else(|| anyhow!("unknown fund {}", r.fund_id))?;
        let st = f.attestations.iter().find(|x| x.attestation.round == r.round).ok_or_else(|| anyhow!("round {} not found", r.round))?;
        if !matches(c, &st.attestation) {
            bail!("the certificate doesn't describe round {} of {}", r.round, r.fund_id);
        }
        if c.checks_passed != c.checks_total || c.checks_total == 0 {
            bail!("a certificate needs every check to pass");
        }
        let r = &mut self.cert_requests[i];
        r.status = "issued".into();
        r.decided_at = Some(now());
        r.certificate = Some(b.certificate);
        Ok(r.clone())
    }

    /// An authority declines a request, with a reason the fund sees.
    pub fn decline_certification(&mut self, b: DeclineBody) -> Result<CertRequest> {
        let i = self.cert_requests.iter().position(|r| r.id == b.request_id).ok_or_else(|| anyhow!("no request {}", b.request_id))?;
        if self.cert_requests[i].authority_key != b.signed.key {
            bail!("this request was sent to another authority");
        }
        let msg = decline_message(&self.id, &b.request_id, &b.signed.key, &b.reason, b.signed.time);
        self.authority_signed(&b.signed, &msg)?;
        if b.reason.trim().is_empty() || b.reason.len() > 500 {
            bail!("give a reason of up to 500 characters");
        }
        let r = &mut self.cert_requests[i];
        if r.status != "pending" {
            bail!("request {} is already {}", r.id, r.status);
        }
        r.status = "declined".into();
        r.reason = Some(b.reason.trim().into());
        r.decided_at = Some(now());
        Ok(r.clone())
    }
}

/// The registry's own certificate key, made on first use. Secret: lives next to registry.json.
pub fn registry_key(d: &DataDir, reg_id: &str) -> Result<IssuerKey> {
    let p = d.registry_key();
    if p.exists() {
        return read_json(&p);
    }
    let k = IssuerKey::generate(&format!("Z-Solvency registry {reg_id}"), "registry");
    crate::store::write_secret_json(&p, &k)?;
    Ok(k)
}

pub fn now() -> String {
    let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    format!("{s}")
}

#[derive(Serialize)]
pub struct FundView<'a> {
    pub id: &'a str,
    /// ZEC, or TAZ on testnet.
    pub unit: &'static str,
    pub key: &'a str,
    pub name: &'a str,
    pub description: &'a str,
    pub obligations: &'a [Obligation],
    pub obligations_zat: u64,
    pub min_held_blocks: u64,
    pub max_proof_age_blocks: u64,
    pub zec_usd: Option<f64>,
    pub price_time: &'a Option<String>,
    pub status: &'static str,
    pub status_text: String,
    pub latest: Option<serde_json::Value>,
    pub rounds: usize,
    /// Set when this fund's most recent submission was refused.
    pub last_refusal: Option<String>,
    /// Accepted rounds and refusals, oldest first (the most recent 30).
    pub past: Vec<PastItem>,
}

#[derive(Serialize, Clone, Debug)]
pub struct PastItem {
    /// Unix seconds.
    pub time: String,
    pub accepted: bool,
    pub round: Option<u64>,
    pub at_height: Option<u64>,
    pub light: &'static str,
    pub text: String,
}

/// A fund's page data. The light follows the design's status rules, first
/// match wins: refused, nothing yet, no longer verifies, no proof for two
/// cadences, late, below obligations, held too briefly, green.
pub fn fund_view<'a>(fund: &'a Fund, ledger: &Ledger, events: &[Event]) -> FundView<'a> {
    let (tip, unit) = (ledger.tip(), ledger.unit());
    let f = &fund.profile;
    let obligations_zat: u64 = f.obligations.iter().fold(0, |t, o| t.saturating_add(o.zat));
    let latest = fund.attestations.last();
    let last_refusal = events.iter().rev().find(|e| e.fund_id == f.id).filter(|e| !e.accepted).map(|e| e.summary.clone());
    let (status, status_text) = match latest {
        None => match &last_refusal {
            Some(r) => ("red", format!("Submission refused: {r}")),
            None => ("none", "No proof published yet".to_string()),
        },
        Some(s) => {
            let a = &s.attestation;
            let age = tip.saturating_sub(a.at_height);
            let cadence = f.max_proof_age_blocks;
            if !s.verdict.ok {
                ("red", "Latest proof failed verification".into())
            } else if a.at_height > tip {
                ("amber", format!("The registry's chain data doesn't reach block {} yet", a.at_height))
            } else if ledger.block(a.at_height).map(|b| b.hash != a.at_block_hash).unwrap_or(true) {
                ("red", format!("Latest proof no longer verifies: block {} changed (a chain reorganisation)", a.at_height))
            } else if cadence > 0 && age > 2 * cadence {
                let text = if cadence == WEEK_BLOCKS { format!("No proof for {} weeks", age / WEEK_BLOCKS) } else { format!("No proof for {}", span(age)) };
                ("red", text)
            } else if cadence > 0 && age > cadence + cadence / 7 {
                // late by more than a seventh of the cadence: a day, for a weekly fund
                ("amber", format!("Last proof is {} old; this fund proves {}", span(age), cadence_words(cadence)))
            } else {
                round_light(f, a, obligations_zat, unit)
            }
        }
    };
    let mut past: Vec<PastItem> = fund
        .attestations
        .iter()
        .map(|s| {
            let (light, text) = round_light(f, &s.attestation, obligations_zat, unit);
            PastItem { time: s.received_at.clone(), accepted: true, round: Some(s.attestation.round), at_height: Some(s.attestation.at_height), light, text }
        })
        .chain(events.iter().filter(|e| e.fund_id == f.id && !e.accepted).map(|e| PastItem {
            time: e.time.clone(),
            accepted: false,
            round: None,
            at_height: None,
            light: "red",
            text: e.summary.clone(),
        }))
        .collect();
    past.sort_by_key(|p| p.time.parse::<u64>().unwrap_or(0));
    let past = past.split_off(past.len().saturating_sub(30));
    let latest = latest.map(|s| {
        let a = &s.attestation;
        serde_json::json!({
            "round": a.round,
            "threshold_zat": a.threshold_zat,
            "since_height": a.since_height,
            "at_height": a.at_height,
            "at_block_hash": a.at_block_hash,
            "registry_nullifier": a.at.registry_nullifiers.first(),
            "registry_nullifiers": a.at.registry_nullifiers,
            "proof_bytes": a.at.proof.len() / 2 + a.since.proof.len() / 2,
            "prove_ms": a.at.prove_ms + a.since.prove_ms,
            "message": a.message,
            "received_at": s.received_at,
            "verdict": s.verdict,
            "blocks_behind_tip": tip.saturating_sub(a.at_height),
        })
    });
    FundView {
        id: &f.id,
        unit,
        key: &fund.key,
        name: &f.name,
        description: &f.description,
        obligations: &f.obligations,
        obligations_zat,
        min_held_blocks: f.min_held_blocks,
        max_proof_age_blocks: f.max_proof_age_blocks,
        zec_usd: f.zec_usd,
        price_time: &f.price_time,
        status,
        status_text,
        latest,
        rounds: fund.attestations.len(),
        last_refusal,
        past,
    }
}

// ------------------------------------------------------------------ HTTP

fn json_resp(code: u16, v: &impl Serialize) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    tiny_http::Response::from_data(serde_json::to_vec(v).unwrap())
        .with_status_code(code)
        .with_header("Content-Type: application/json".parse::<tiny_http::Header>().unwrap())
        .with_header("Access-Control-Allow-Origin: *".parse::<tiny_http::Header>().unwrap())
}

/// Rebuilding roots at a new height is the one request that costs seconds,
/// and any registered fund can cause it (registration may be free). Across
/// all clients, only this many such heights per hour; nothing else waits.
struct RebuildBudget {
    per_hour: f64,
    tokens: f64,
    at: std::time::Instant,
}

impl RebuildBudget {
    fn new(per_hour: u32) -> Self {
        RebuildBudget { per_hour: per_hour as f64, tokens: per_hour as f64, at: std::time::Instant::now() }
    }
    /// Allow building roots at these heights, or say how long to wait.
    fn allow(&mut self, ledger: &Ledger, heights: &[u64]) -> std::result::Result<(), String> {
        let mut new: Vec<u64> = heights.iter().copied().filter(|&h| !ledger.roots_are_cheap(h)).collect();
        new.sort();
        new.dedup();
        if new.is_empty() || self.per_hour <= 0.0 {
            return Ok(());
        }
        let now = std::time::Instant::now();
        self.tokens = (self.tokens + now.duration_since(self.at).as_secs_f64() * self.per_hour / 3600.0).min(self.per_hour);
        self.at = now;
        if self.tokens >= new.len() as f64 {
            self.tokens -= new.len() as f64;
            Ok(())
        } else {
            let wait = ((new.len() as f64 - self.tokens) * 3600.0 / self.per_hour).ceil();
            Err(format!("busy: roots at new heights are rebuilt at most {} times an hour; try again in {wait} s", self.per_hour))
        }
    }
}

/// One API request. `budget` is the worker's: only it handles POSTs.
fn handle(d: &DataDir, reg_id: &str, ledger: &Ledger, budget: Option<&mut RebuildBudget>, method: &str, url: &str, body: &str) -> Result<(u16, serde_json::Value)> {
    let path = url.split('?').next().unwrap_or(url);
    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
    let tip = ledger.tip();
    let unit = ledger.unit();
    let mut reg = Registry::load_or_new(d, reg_id)?;
    use serde_json::json;
    Ok(match (method, parts.as_slice()) {
        ("GET", ["api", "mode"]) => (200, json!({"mode": "registry"})),
        // Chain facts only: funds are unlisted, reachable by their share link.
        ("GET", ["api", "state"]) => {
            (200, json!({ "registry": reg.id, "network": ledger.network, "pool": ledger.pool, "unit": unit, "tip": tip,
                          "tip_hash": ledger.block(tip)?.hash, "registry_key": registry_key(d, reg_id)?.public()? }))
        }
        ("GET", ["api", "links", link]) => match reg.by_link(link) {
            Some(f) => (200, serde_json::to_value(fund_view(f, ledger, &reg.events))?),
            None => (404, json!({"error": "no fund at this link"})),
        },
        ("GET", ["api", "links", link, "attestation"]) => match reg.by_link(link).and_then(|f| f.attestations.last()) {
            Some(s) => (200, serde_json::to_value(&s.attestation)?),
            None => (404, json!({"error": "no attestation"})),
        },
        ("POST", ["api", "links", link, "verify"]) => match reg.by_link(link).and_then(|f| f.attestations.last()) {
            Some(s) => {
                let v = verify(&s.attestation, ledger);
                (200, serde_json::to_value(v)?)
            }
            None => (404, json!({"error": "no attestation"})),
        },
        ("GET", ["api", "links", link, "viewing-key"]) => match reg.by_link(link) {
            Some(f) => {
                let p = d.public().join(format!("viewing-key-{}.json", f.profile.id));
                if p.exists() { (200, read_json(&p)?) } else { (200, serde_json::Value::Null) }
            }
            None => (404, json!({"error": "no fund at this link"})),
        },
        // Only heights already computed here: anyone may ask, so this must stay cheap.
        ("GET", ["api", "roots", h]) => {
            let h: u64 = h.parse()?;
            if ledger.roots_are_cheap(h) {
                (200, serde_json::to_value(ledger.roots(h)?)?)
            } else {
                (404, json!({"error": format!("roots at block {h} are not computed on this registry; rebuild them yourself with `zsolv roots --height {h}`")}))
            }
        }
        ("POST", ["api", "funds"]) => {
            let u: ProfileUpdate = serde_json::from_str(body).map_err(|e| anyhow!("bad profile update: {e}"))?;
            // Only a profile signed by the fund's key gets here, so only the fund learns its link.
            let id = reg.register(u)?;
            reg.ensure_links();
            reg.save(d)?;
            (200, json!({"ok": true, "id": id, "link": reg.funds[&id].link}))
        }
        ("POST", ["api", "attest"]) => {
            let a: Attestation = serde_json::from_str(body).map_err(|e| anyhow!("bad attestation: {e}"))?;
            // Cheap refusals first; then only the fund's own signed rounds can spend the budget.
            if a.since.height != a.since_height || a.at.height != a.at_height {
                bail!("heights inconsistent");
            }
            let budget = budget.ok_or_else(|| anyhow!("submissions are handled by the worker"))?;
            if let Some(f) = reg.funds.get(&a.fund_id) {
                if a.fund_key == f.key && verify_sig(&f.key, &a.message, &a.fund_signature).is_ok() {
                    if let Err(e) = budget.allow(ledger, &[a.since_height, a.at_height]) {
                        return Ok((503, json!({"error": e})));
                    }
                }
            }
            let fund_id = a.fund_id.clone();
            let (accepted, reason, verdict) = reg.submit(a, ledger)?;
            // Every accepted round gets the registry's own certificate, kept for the fund only.
            let certificate = if accepted { Some(reg.certify_latest(&fund_id, &registry_key(d, reg_id)?)?) } else { None };
            reg.save(d)?;
            (if accepted { 200 } else { 422 }, json!({"accepted": accepted, "reason": reason, "verdict": verdict, "certificate": certificate}))
        }
        // Approved authorities are public: a fund picks one to ask.
        ("GET", ["api", "authorities"]) => (200, serde_json::to_value(reg.authorities.values().collect::<Vec<_>>())?),
        ("POST", ["api", "certificates", "request"]) => {
            let r = reg.request_certification(serde_json::from_str(body).map_err(|e| anyhow!("bad request: {e}"))?)?;
            reg.save(d)?;
            (200, serde_json::to_value(r)?)
        }
        ("POST", ["api", "certificates", "fund"]) => {
            let b: FundReadBody = serde_json::from_str(body).map_err(|e| anyhow!("bad request: {e}"))?;
            (200, reg.fund_certificates(&b)?)
        }
        ("POST", ["api", "certificates", "inbox"]) => {
            let s: Signed = serde_json::from_str(body).map_err(|e| anyhow!("bad request: {e}"))?;
            (200, reg.authority_inbox(&s)?)
        }
        ("POST", ["api", "certificates", "issue"]) => {
            let r = reg.issue_certificate(serde_json::from_str(body).map_err(|e| anyhow!("bad certificate: {e}"))?)?;
            reg.save(d)?;
            (200, serde_json::to_value(r)?)
        }
        ("POST", ["api", "certificates", "decline"]) => {
            let r = reg.decline_certification(serde_json::from_str(body).map_err(|e| anyhow!("bad request: {e}"))?)?;
            reg.save(d)?;
            (200, serde_json::to_value(r)?)
        }
        _ => (404, json!({"error": "not found"})),
    })
}

/// Run a handler; an error is a 400 and a panic a 500, never a dead server.
pub(crate) fn guarded(f: impl FnOnce() -> Result<(u16, serde_json::Value)>) -> (u16, serde_json::Value) {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => (400, serde_json::json!({"error": e.to_string()})),
        Err(_) => (500, serde_json::json!({"error": "internal error"})),
    }
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

/// Serve the built web app (web/dist). Unknown paths fall back to index.html.
pub(crate) fn static_file(web: &Path, url_path: &str) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let rel = url_path.trim_start_matches('/');
    let safe = !rel.split('/').any(|seg| seg == ".." || seg.contains('\\'));
    let mut path = if rel.is_empty() || !safe { web.join("index.html") } else { web.join(rel) };
    if !path.is_file() {
        path = web.join("index.html");
    }
    let header = |s: String| s.parse::<tiny_http::Header>().unwrap();
    match std::fs::read(&path) {
        Ok(bytes) => {
            let cache = if path.starts_with(web.join("assets")) { "public, max-age=31536000, immutable" } else { "no-cache" };
            tiny_http::Response::from_data(bytes)
                .with_header(header(format!("Content-Type: {}", content_type(&path))))
                .with_header(header(format!("Cache-Control: {cache}")))
        }
        Err(_) => tiny_http::Response::from_data(
            b"Web UI not built. Run: npm --prefix web ci && npm --prefix web run build".to_vec(),
        )
        .with_status_code(503)
        .with_header(header("Content-Type: text/plain; charset=utf-8".into())),
    }
}

/// Chain data kept in memory (testnet's is ~100 MB), shared by the reader and
/// the worker, and reloaded when the file changes, e.g. after
/// `zsolv testnet sync` runs beside the server.
struct LedgerCache {
    d: DataDir,
    stamp: Option<(PathBuf, u64, std::time::SystemTime)>,
    ledger: Option<Arc<Ledger>>,
}

type SharedChain = Arc<Mutex<LedgerCache>>;

fn chain(c: &SharedChain) -> Result<Arc<Ledger>> {
    let mut c = c.lock().unwrap_or_else(|e| e.into_inner());
    let p = c.d.ledger();
    let m = std::fs::metadata(&p).map_err(|_| anyhow!("no chain data yet in {}: run `zsolv testnet sync` (or `zsolv devnet init`)", c.d.0.join("chain").display()))?;
    let stamp = Some((p.clone(), m.len(), m.modified()?));
    if c.ledger.is_none() || stamp != c.stamp {
        c.ledger = Some(Arc::new(Ledger::load(&p)?));
        c.stamp = stamp;
    }
    Ok(c.ledger.clone().unwrap())
}

/// Limits for requests that cost real work: every POST (a submission or a
/// Verify checks two proofs, about a second; a profile update a signature).
pub struct RateLimit {
    /// Per client.
    pub per_minute: u32,
    /// Root rebuilds at new heights per hour, across all clients.
    pub rebuilds_per_hour: u32,
    /// Take the client from the last X-Forwarded-For entry, the one your own
    /// proxy appended (only behind such a proxy; earlier entries are forgeable).
    pub trust_proxy: bool,
}

struct Buckets {
    per_minute: f64,
    map: std::collections::HashMap<String, (f64, std::time::Instant)>,
}

impl Buckets {
    /// Ok, or the seconds to wait.
    fn take(&mut self, client: &str) -> std::result::Result<(), u64> {
        if self.per_minute <= 0.0 {
            return Ok(());
        }
        let now = std::time::Instant::now();
        if self.map.len() > 10_000 {
            let cap = self.per_minute;
            let rate = cap / 60.0;
            self.map.retain(|_, (t, at)| *t + now.duration_since(*at).as_secs_f64() * rate < cap);
        }
        let cap = self.per_minute;
        let (tokens, at) = self.map.entry(client.to_string()).or_insert((cap, now));
        *tokens = (*tokens + now.duration_since(*at).as_secs_f64() * cap / 60.0).min(cap);
        *at = now;
        if *tokens >= 1.0 {
            *tokens -= 1.0;
            Ok(())
        } else {
            Err(((1.0 - *tokens) * 60.0 / cap).ceil() as u64)
        }
    }
}

struct Job {
    rq: tiny_http::Request,
    method: String,
    url: String,
}

/// POSTs waiting for the worker; beyond this the registry answers "busy".
const QUEUE: usize = 16;

pub fn serve(d: DataDir, reg_id: String, addr: &str, web: PathBuf, limit: RateLimit) -> Result<()> {
    let mut reg = Registry::load_or_new(&d, &reg_id)?;
    if reg.ensure_links() {
        reg.save(&d)?;
    }
    // Make the certificate key now, before requests race to create it.
    registry_key(&d, &reg_id)?;
    let server = tiny_http::Server::http(addr).map_err(|e| anyhow!("{e}"))?;
    eprintln!(
        "Z-Solvency registry {reg_id} on http://{addr} (POSTs: {} per minute per client; new-height root rebuilds: {} per hour)",
        limit.per_minute, limit.rebuilds_per_hour
    );
    if !web.join("index.html").is_file() {
        eprintln!("warning: no web build at {} (npm --prefix web ci && npm --prefix web run build)", web.display());
    }
    let shared: SharedChain = Arc::new(Mutex::new(LedgerCache { d: d.clone(), stamp: None, ledger: None }));
    let mut buckets = Buckets { per_minute: limit.per_minute as f64, map: Default::default() };
    // Warm the proving system's verifying key so the first Verify is quick.
    std::thread::spawn(|| {
        let _ = voting_circuits::delegation::warm_delegation_keys();
    });

    let (tx, rx) = std::sync::mpsc::sync_channel::<Job>(QUEUE);
    {
        let (d, reg_id, shared) = (d.clone(), reg_id.clone(), shared.clone());
        let mut budget = RebuildBudget::new(limit.rebuilds_per_hour);
        std::thread::spawn(move || {
            for mut job in rx {
                let mut body = String::new();
                let _ = job.rq.as_reader().take(8 << 20).read_to_string(&mut body);
                let t0 = std::time::Instant::now();
                let (code, v) = guarded(|| {
                    let l = chain(&shared)?;
                    handle(&d, &reg_id, &l, Some(&mut budget), &job.method, &job.url, &body)
                });
                eprintln!("{} {} {code} {:?}", job.method, job.url, t0.elapsed());
                let _ = job.rq.respond(json_resp(code, &v));
            }
        });
    }

    for rq in server.incoming_requests() {
        let method = rq.method().as_str().to_string();
        let url = rq.url().to_string();
        let path = url.split('?').next().unwrap_or("/").to_string();
        if !path.starts_with("/api/") {
            let resp = if method == "GET" || method == "HEAD" {
                static_file(&web, &path)
            } else {
                json_resp(405, &serde_json::json!({"error": "method not allowed"}))
            };
            let _ = rq.respond(resp);
            continue;
        }
        if method != "POST" {
            let t0 = std::time::Instant::now();
            let (code, v) = guarded(|| {
                let l = chain(&shared)?;
                handle(&d, &reg_id, &l, None, &method, &url, "")
            });
            eprintln!("{method} {url} {code} {:?}", t0.elapsed());
            let _ = rq.respond(json_resp(code, &v));
            continue;
        }
        let fwd = limit
            .trust_proxy
            .then(|| rq.headers().iter().find(|h| h.field.equiv("X-Forwarded-For")).map(|h| h.value.as_str().rsplit(',').next().unwrap_or("").trim().to_string()))
            .flatten();
        let client = fwd.unwrap_or_else(|| rq.remote_addr().map(|a| a.ip().to_string()).unwrap_or_default());
        if let Err(wait) = buckets.take(&client) {
            let resp = json_resp(429, &serde_json::json!({"error": format!("too many requests: try again in {wait} s")}))
                .with_header(format!("Retry-After: {wait}").parse::<tiny_http::Header>().unwrap());
            eprintln!("{method} {url} 429 for {client}");
            let _ = rq.respond(resp);
            continue;
        }
        match tx.try_send(Job { rq, method, url }) {
            Ok(()) => {}
            Err(std::sync::mpsc::TrySendError::Full(job)) | Err(std::sync::mpsc::TrySendError::Disconnected(job)) => {
                eprintln!("{} {} 503 queue full", job.method, job.url);
                let _ = job.rq.respond(
                    json_resp(503, &serde_json::json!({"error": "busy: too many requests in progress; try again shortly"}))
                        .with_header("Retry-After: 10".parse::<tiny_http::Header>().unwrap()),
                );
            }
        }
    }
    Ok(())
}
