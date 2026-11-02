//! Build and check Z-Solvency attestations.
//!
//! An attestation is two delegation proofs (from Valar's `voting-circuits`,
//! Halo 2) over the same reserve notes: one against the Ironwood roots at
//! block X ("since") and one at block H ("at"). Each proof shows the notes
//! are in the tree and unspent at that height, and commits to their total
//! value in 0.125 ZEC steps inside `van_comm`, which the fund opens. With
//! `vote_round_id` fixed to the registry id, each note's alternate nullifier
//! (`gov_null`) is a stable registry nullifier: equal sets at X and H mean
//! the same notes were held throughout; the registry refuses them for a
//! second fund. A RedPallas signature under each proof's `rk` over the round
//! message proves the fund holds the spending key, not just a viewing key.
//! The fund's own key (`fundkey`) also signs it, so the registry can tie the
//! round to the fund that registered that key.

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use voting_circuits::delegation::{
    build_delegation_bundle, create_delegation_proof, derive_nullifier_domain, van_commitment_hash,
    verify_delegation_proof, DelegationBuildError, ImtProvider, Instance, PaddedNoteData,
    PrecomputedRandomness, RealNoteInput,
};
use voting_circuits::ff::{Field, PrimeField};
use voting_circuits::group::Curve;
use voting_crypto_deps::pasta_curves::arithmetic::CurveAffine;
use voting_circuits::rand::rngs::OsRng;
use voting_circuits::rand::Rng;
use voting_crypto_deps::orchard::{
    keys::{FullViewingKey, Scope, SpendAuthorizingKey, SpendingKey},
    note::{Nullifier, RandomSeed, Rho},
    primitives::redpallas::{Signature, SpendAuth, VerificationKey},
};
use voting_crypto_deps::pasta_curves::pallas;

use crate::fundkey::{verify_sig, FundKey};
use crate::ledger::Ledger;
use crate::util::{fhex, hexf, label_to_field, zec, STEP_ZAT};
use crate::wallet::{Wallet, WalletNote};

pub const VERSION: &str = "z-solvency/v2";
pub const SLOTS: usize = 5;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VanOpening {
    pub g_d_x: String,
    pub pk_d_x: String,
    pub steps: u64,
    pub rand: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Snapshot {
    pub height: u64,
    /// Copies for convenience only: verifiers rebuild roots from chain data.
    pub nc_root: String,
    pub nf_imt_root: String,
    pub nf_signed: String,
    pub rk: String,
    pub cmx_new: String,
    pub van_comm: String,
    pub registry_nullifiers: Vec<String>,
    pub van_opening: VanOpening,
    pub proof: String,
    pub signature: String,
    pub prove_ms: u128,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Attestation {
    pub version: String,
    pub registry: String,
    pub fund_id: String,
    /// The fund's public key, as registered. Signs the round message.
    pub fund_key: String,
    pub round: u64,
    pub threshold_zat: u64,
    pub since_height: u64,
    pub at_height: u64,
    pub at_block_hash: String,
    pub message: String,
    pub fund_signature: String,
    pub since: Snapshot,
    pub at: Snapshot,
}

impl Attestation {
    pub fn registry_nullifiers(&self) -> &[String] {
        &self.at.registry_nullifiers
    }
}

pub fn round_message(a: &Attestation) -> String {
    format!(
        "{VERSION} attestation\nregistry: {}\nfund: {}\nfund_key: {}\nround: {}\nthreshold_zat: {}\nsince_height: {}\nat_height: {}\nat_block_hash: {}\nregistry_nullifiers: {}\nvan_comm_since: {}\nvan_comm_at: {}\n",
        a.registry,
        a.fund_id,
        a.fund_key,
        a.round,
        a.threshold_zat,
        a.since_height,
        a.at_height,
        a.at_block_hash,
        a.at.registry_nullifiers.join(","),
        a.since.van_comm,
        a.at.van_comm,
    )
}

pub struct ProveRequest<'a> {
    pub wallet: &'a Wallet,
    pub ledger: &'a Ledger,
    pub registry: String,
    pub fund_id: String,
    pub fund_key: &'a FundKey,
    pub round: u64,
    pub threshold_zat: u64,
    pub since: Option<u64>,
    pub at: u64,
    /// Demo only: label the attestation with this since-height while proving
    /// against the real (later) one. Verification must catch it.
    pub cheat_claim_since: Option<u64>,
    pub log: &'a dyn Fn(&str),
}

fn base_x(p: &pallas::Point) -> pallas::Base {
    *p.to_affine().coordinates().unwrap().x()
}

fn random_rseed(rho: &Rho) -> [u8; 32] {
    loop {
        let mut b = [0u8; 32];
        OsRng.fill_bytes(&mut b);
        if RandomSeed::from_bytes(b, rho).is_some().into() {
            return b;
        }
    }
}

fn padding_notes(n: usize) -> Vec<PaddedNoteData> {
    (0..n)
        .map(|_| {
            let rho_bytes = pallas::Base::random(&mut OsRng).to_repr();
            let rho = Rho::from_bytes(&rho_bytes).unwrap();
            PaddedNoteData { rho: rho_bytes, rseed: random_rseed(&rho) }
        })
        .collect()
}

struct Built {
    snap: Snapshot,
    rsk: voting_crypto_deps::orchard::primitives::redpallas::SigningKey<SpendAuth>,
}

fn build_snapshot(
    sk: &SpendingKey,
    fvk: &FullViewingKey,
    notes: &[&WalletNote],
    ledger: &Ledger,
    height: u64,
    round_id: pallas::Base,
    padding: &[PaddedNoteData],
) -> Result<Built> {
    let tree = ledger.note_tree(height)?;
    let imt = ledger.nullifier_imt(height)?;
    let nc_root = tree.root();

    let mut inputs = vec![];
    for wn in notes {
        let note = wn.note()?;
        let merkle_path = tree.path(wn.position)?;
        let nf = note.nullifier(fvk);
        let imt_proof = imt
            .non_membership_proof(voting_crypto_deps::CircuitValue::circuit_value(&nf))
            .map_err(|e| anyhow!("at block {height}: {}", e.0))?;
        let scope = if wn.internal { Scope::Internal } else { Scope::External };
        inputs.push(RealNoteInput { note, fvk: fvk.clone(), merkle_path, imt_proof, scope });
    }
    let total: u64 = notes.iter().map(|n| n.value_zat).sum();

    // The VAN's output address is a throwaway key: opening it reveals nothing.
    let throwaway = FullViewingKey::from(&voting_crypto_deps::random_spending_key(&mut OsRng));
    let output_recipient = throwaway.address_at(0u32, Scope::External);
    let alpha = pallas::Scalar::random(&mut OsRng);
    let van_comm_rand = pallas::Base::random(&mut OsRng);

    let mut attempt = 0;
    let bundle = loop {
        attempt += 1;
        let pre = PrecomputedRandomness {
            padded_notes: padding.to_vec(),
            rseed_signed: { let mut b = [0u8; 32]; OsRng.fill_bytes(&mut b); b },
            rseed_output: { let mut b = [0u8; 32]; OsRng.fill_bytes(&mut b); b },
        };
        let inputs_try = inputs
            .iter()
            .map(|i| RealNoteInput {
                note: i.note,
                fvk: i.fvk.clone(),
                merkle_path: i.merkle_path.clone(),
                imt_proof: i.imt_proof.clone(),
                scope: i.scope,
            })
            .collect();
        match build_delegation_bundle(inputs_try, fvk, alpha, output_recipient, round_id, nc_root, van_comm_rand, &imt, &mut OsRng, Some(&pre)) {
            Ok(b) => break b,
            Err(DelegationBuildError::InvalidPrecomputedRseed { .. }) | Err(DelegationBuildError::InvalidPrecomputedNote { .. }) if attempt < 32 => continue,
            Err(e) => bail!("building the proof at block {height}: {e}"),
        }
    };

    let t0 = std::time::Instant::now();
    let proof = create_delegation_proof(bundle.circuit, &bundle.instance)
        .map_err(|e| anyhow!("proving at block {height} failed: {e:?}"))?;
    let prove_ms = t0.elapsed().as_millis();

    let rsk = SpendAuthorizingKey::from(sk).randomize(&voting_crypto_deps::spend_auth_randomizer(&alpha));
    let rk: [u8; 32] = (&VerificationKey::from(&rsk)).into();
    let inst = &bundle.instance;

    let g_d_x = base_x(&voting_crypto_deps::diversified_base(&output_recipient));
    let pk_d_x = base_x(&voting_crypto_deps::transmission_point(&voting_crypto_deps::transmission_key(&output_recipient)));
    let steps = total / STEP_ZAT;
    debug_assert_eq!(van_commitment_hash(g_d_x, pk_d_x, pallas::Base::from(steps), round_id, van_comm_rand), inst.van_comm);

    Ok(Built {
        snap: Snapshot {
            height,
            nc_root: fhex(&inst.nc_root),
            nf_imt_root: fhex(&inst.nf_imt_root),
            nf_signed: hex::encode(inst.nf_signed.to_bytes()),
            rk: hex::encode(rk),
            cmx_new: fhex(&inst.cmx_new),
            van_comm: fhex(&inst.van_comm),
            registry_nullifiers: inst.gov_null.iter().map(fhex).collect(),
            van_opening: VanOpening { g_d_x: fhex(&g_d_x), pk_d_x: fhex(&pk_d_x), steps, rand: fhex(&van_comm_rand) },
            proof: hex::encode(proof),
            signature: String::new(),
            prove_ms,
        },
        rsk,
    })
}

pub fn prove(req: ProveRequest) -> Result<Attestation> {
    let log = req.log;
    let t = req.threshold_zat;
    let unit = req.ledger.unit();
    if t == 0 || t % STEP_ZAT != 0 {
        bail!("threshold must be a positive multiple of 0.125 {unit} (got {} {unit})", zec(t));
    }
    if req.fund_key.fund_id != req.fund_id {
        bail!("the fund key is for {}, not {}", req.fund_key.fund_id, req.fund_id);
    }
    let h = req.at;
    req.ledger.block(h)?;
    let reserve: Vec<&WalletNote> = req.wallet.unspent_at(h).filter(|n| n.reserve).collect();
    if reserve.is_empty() {
        bail!("wallet {} has no unspent reserve note at block {h} (make one with `zsolv reserve`)", req.wallet.name);
    }
    if reserve.len() > SLOTS {
        bail!("at most {SLOTS} reserve notes fit in one proof; consolidate them");
    }
    let total: u64 = reserve.iter().map(|n| n.value_zat).sum();
    if total < t {
        bail!("reserve notes hold {} {unit}, below the threshold {} {unit}", zec(total), zec(t));
    }
    if total >= t + STEP_ZAT {
        bail!(
            "reserve notes hold {} {unit}; the proof would reveal {} {unit}, more than T = {} {unit}. Keep a reserve note of exactly T.",
            zec(total), zec(total / STEP_ZAT * STEP_ZAT), zec(t)
        );
    }
    let earliest = reserve.iter().map(|n| n.height).max().unwrap();
    let since = match req.since {
        Some(x) if x < earliest => bail!(
            "the reserve notes only exist from block {earliest}: you can prove \"held since {earliest}\", not since {x}"
        ),
        Some(x) if x > h => bail!("since-height {x} is after at-height {h}"),
        Some(x) => x,
        None => earliest,
    };

    let sk = req.wallet.sk()?;
    let fvk = req.wallet.fvk()?;
    let round_id = label_to_field(&req.registry);
    // Same padding notes in both proofs, so all five registry nullifiers match.
    let padding = padding_notes(SLOTS - reserve.len());

    log(&format!("proving {} reserve note(s) at block {since} (since)…", reserve.len()));
    let bx = build_snapshot(&sk, &fvk, &reserve, req.ledger, since, round_id, &padding)?;
    log(&format!("  done in {:.1}s", bx.snap.prove_ms as f64 / 1000.0));
    log(&format!("proving at block {h} (at)…"));
    let bh = build_snapshot(&sk, &fvk, &reserve, req.ledger, h, round_id, &padding)?;
    log(&format!("  done in {:.1}s", bh.snap.prove_ms as f64 / 1000.0));

    let mut a = Attestation {
        version: VERSION.into(),
        registry: req.registry.clone(),
        fund_id: req.fund_id.clone(),
        fund_key: req.fund_key.public()?,
        round: req.round,
        threshold_zat: t,
        since_height: req.cheat_claim_since.unwrap_or(since),
        at_height: h,
        at_block_hash: req.ledger.block(h)?.hash.clone(),
        message: String::new(),
        fund_signature: String::new(),
        since: bx.snap,
        at: bh.snap,
    };
    if let Some(fake) = req.cheat_claim_since {
        a.since.height = fake;
    }
    a.message = round_message(&a);
    let sx: [u8; 64] = (&bx.rsk.sign(OsRng, a.message.as_bytes())).into();
    let sh: [u8; 64] = (&bh.rsk.sign(OsRng, a.message.as_bytes())).into();
    a.since.signature = hex::encode(sx);
    a.at.signature = hex::encode(sh);
    a.fund_signature = req.fund_key.sign(&a.message)?;
    Ok(a)
}

// ---------------------------------------------------------------- verify

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Verdict {
    pub ok: bool,
    pub fund_id: String,
    pub threshold_zat: u64,
    pub since_height: u64,
    pub at_height: u64,
    pub tip: u64,
    pub checks: Vec<Check>,
}

struct Checks(Vec<Check>);
impl Checks {
    fn add(&mut self, name: &str, r: Result<String>) -> bool {
        let (ok, detail) = match r {
            Ok(d) => (true, d),
            Err(e) => (false, e.to_string()),
        };
        self.0.push(Check { name: name.into(), ok, detail });
        ok
    }
}

fn check_snapshot(a: &Attestation, s: &Snapshot, label: &str, ledger: &Ledger, round_id: pallas::Base, c: &mut Checks) {
    let roots = ledger.roots(s.height);
    let roots = match roots {
        Ok(r) => r,
        Err(e) => {
            c.add(&format!("{label}: roots at block {}", s.height), Err(e));
            return;
        }
    };
    c.add(
        &format!("{label}: roots rebuilt from chain data at block {}", s.height),
        Ok(format!("{} commitments, {} nullifiers", roots.commitments, roots.nullifiers)),
    );

    let inst = (|| -> Result<Instance> {
        let nf = Option::from(Nullifier::from_bytes(&crate::util::b32(&s.nf_signed)?)).ok_or_else(|| anyhow!("bad nf_signed"))?;
        let rk = VerificationKey::<SpendAuth>::try_from(crate::util::b32(&s.rk)?).map_err(|_| anyhow!("bad rk"))?;
        let gn: Vec<pallas::Base> = s.registry_nullifiers.iter().map(|x| hexf(x)).collect::<Result<_>>()?;
        if gn.len() != SLOTS {
            bail!("expected {SLOTS} registry nullifiers");
        }
        Instance::from_parts(
            nf,
            rk,
            hexf(&s.cmx_new)?,
            hexf(&s.van_comm)?,
            round_id,
            hexf(&roots.nc_root)?,
            hexf(&roots.nf_imt_root)?,
            gn.try_into().unwrap(),
            derive_nullifier_domain(round_id),
        )
        .map_err(|e| anyhow!("{e}"))
    })();
    let inst = match inst {
        Ok(i) => i,
        Err(e) => {
            c.add(&format!("{label}: public inputs well-formed"), Err(e));
            return;
        }
    };

    c.add(&format!("{label}: Halo 2 proof verifies against those roots"), (|| {
        let proof = hex::decode(&s.proof)?;
        verify_delegation_proof(&proof, &inst).map_err(|_| {
            anyhow!("proof does not verify against block {}: the notes were not in the tree or already spent then, or the proof was altered", s.height)
        })?;
        Ok(format!("{} byte proof", proof.len()))
    })());

    c.add(&format!("{label}: value commitment opens to ≥ T"), (|| {
        let o = &s.van_opening;
        let vc = van_commitment_hash(hexf(&o.g_d_x)?, hexf(&o.pk_d_x)?, pallas::Base::from(o.steps), round_id, hexf(&o.rand)?);
        if vc != inst.van_comm {
            bail!("opening does not match van_comm");
        }
        let proven = o.steps * STEP_ZAT;
        if proven < a.threshold_zat {
            bail!("notes prove only {} {unit} < T = {} {unit}", zec(proven), zec(a.threshold_zat), unit = ledger.unit());
        }
        Ok(format!("at least {} {}", zec(proven), ledger.unit()))
    })());

    c.add(&format!("{label}: spend-auth signature over the round message"), (|| {
        let rk = VerificationKey::<SpendAuth>::try_from(crate::util::b32(&s.rk)?).map_err(|_| anyhow!("bad rk"))?;
        let sig: [u8; 64] = hex::decode(&s.signature)?.try_into().map_err(|_| anyhow!("bad signature length"))?;
        rk.verify(a.message.as_bytes(), &Signature::from(sig)).map_err(|_| anyhow!("signature invalid: whoever made this lacks the spending key"))?;
        Ok("signed by the notes' spending key".into())
    })());

}

pub fn verify(a: &Attestation, ledger: &Ledger) -> Verdict {
    let mut c = Checks(vec![]);
    let round_id = label_to_field(&a.registry);

    c.add("chain data hashes link up", ledger.check_links().map(|_| format!("tip {}", ledger.tip())));
    c.add("format and round message", (|| {
        if a.version != VERSION {
            bail!("unknown version {}", a.version);
        }
        if a.message != round_message(a) {
            bail!("the signed message does not match the attestation fields");
        }
        if a.since.height != a.since_height || a.at.height != a.at_height || a.since_height > a.at_height {
            bail!("heights inconsistent");
        }
        if a.threshold_zat == 0 || a.threshold_zat % STEP_ZAT != 0 {
            bail!("threshold must be a positive multiple of 0.125 {}", ledger.unit());
        }
        Ok(format!("round {}, T = {} {}", a.round, zec(a.threshold_zat), ledger.unit()))
    })());
    c.add("fund key signature over the round message", (|| {
        verify_sig(&a.fund_key, &a.message, &a.fund_signature)
            .map_err(|_| anyhow!("fund signature invalid: not signed by the fund key it names"))?;
        Ok(format!("signed by fund key {}", crate::util::short(&a.fund_key)))
    })());
    c.add("block hash at H matches the chain", (|| {
        let b = ledger.block(a.at_height)?;
        if b.hash != a.at_block_hash {
            bail!("block {} hash differs: proof made for another chain or fork", a.at_height);
        }
        Ok(crate::util::short(&b.hash))
    })());

    check_snapshot(a, &a.since, "since", ledger, round_id, &mut c);
    check_snapshot(a, &a.at, "at", ledger, round_id, &mut c);

    c.add("same notes at X and H (held throughout)", (|| {
        if a.since.registry_nullifiers != a.at.registry_nullifiers {
            bail!("registry nullifiers differ between X and H: different notes were proven");
        }
        let mut s = a.at.registry_nullifiers.clone();
        s.sort();
        s.dedup();
        if s.len() != SLOTS {
            bail!("a note is counted twice inside one proof");
        }
        Ok(format!("held since block {}", a.since_height))
    })());

    let ok = c.0.iter().all(|x| x.ok);
    Verdict {
        ok,
        fund_id: a.fund_id.clone(),
        threshold_zat: a.threshold_zat,
        since_height: a.since_height,
        at_height: a.at_height,
        tip: ledger.tip(),
        checks: c.0,
    }
}
