//! The public registry: stores fund profiles and attestations, verifies each
//! attestation against chain data, and refuses a registry nullifier that is
//! already counted by another fund. Serves the reserve pages.

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::attest::{verify, Attestation, Verdict};
use crate::ledger::Ledger;
use crate::store::{read_json, write_json, DataDir};
use crate::util::{short, zec};

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
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Fund {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub obligations: Vec<Obligation>,
    /// Blocks the reserve must have been held to count as "held", e.g. 4 weeks.
    #[serde(default)]
    pub min_held_blocks: u64,
    #[serde(default)]
    pub zec_usd: Option<f64>,
    #[serde(default)]
    pub price_time: Option<String>,
    #[serde(default)]
    pub attestations: Vec<StoredAttestation>,
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
}

impl Registry {
    pub fn new(id: &str) -> Self {
        Registry { id: id.into(), funds: BTreeMap::new(), nullifier_owner: BTreeMap::new(), events: vec![] }
    }
    pub fn load_or_new(d: &DataDir, id: &str) -> Result<Self> {
        if d.registry().exists() { read_json(&d.registry()) } else { Ok(Self::new(id)) }
    }
    pub fn save(&self, d: &DataDir) -> Result<()> {
        write_json(&d.registry(), self)
    }

    pub fn upsert_fund(&mut self, mut f: Fund) {
        if let Some(old) = self.funds.get(&f.id) {
            f.attestations = old.attestations.clone();
        }
        self.funds.insert(f.id.clone(), f);
    }

    fn event(&mut self, fund_id: &str, accepted: bool, summary: String) {
        self.events.push(Event { time: now(), fund_id: fund_id.into(), accepted, summary });
    }

    /// Verify and record an attestation. Returns (accepted, reason, verdict).
    pub fn submit(&mut self, a: Attestation, ledger: &Ledger) -> Result<(bool, String, Option<Verdict>)> {
        let fund_id = a.fund_id.clone();
        let Some(fund) = self.funds.get(&fund_id) else {
            bail!("unknown fund {fund_id}: register its profile first");
        };
        if a.registry != self.id {
            let r = format!("attestation is for registry {}, this is {}", a.registry, self.id);
            self.event(&fund_id, false, r.clone());
            return Ok((false, r, None));
        }
        if let Some(last) = fund.attestations.last() {
            if a.round <= last.attestation.round {
                let r = format!("round {} is not newer than round {} already published", a.round, last.attestation.round);
                self.event(&fund_id, false, r.clone());
                return Ok((false, r, None));
            }
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
                    let owner_name = self.funds.get(owner).map(|f| f.name.clone()).unwrap_or(owner.clone());
                    let r = format!("note already counted by {owner_name} (registry nullifier {})", short(n));
                    self.event(&fund_id, false, r.clone());
                    return Ok((false, r, Some(v)));
                }
            }
        }
        for n in a.registry_nullifiers() {
            self.nullifier_owner.insert(n.clone(), fund_id.clone());
        }
        let summary = format!(
            "round {}: at least {} ZEC, held since block {}, checked at block {}",
            a.round, zec(a.threshold_zat), a.since_height, a.at_height
        );
        self.event(&fund_id, true, summary.clone());
        self.funds.get_mut(&fund_id).unwrap().attestations.push(StoredAttestation { received_at: now(), attestation: a, verdict: v.clone() });
        Ok((true, summary, Some(v)))
    }
}

pub fn now() -> String {
    let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    format!("{s}")
}

#[derive(Serialize)]
pub struct FundView<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub description: &'a str,
    pub obligations: &'a [Obligation],
    pub obligations_zat: u64,
    pub min_held_blocks: u64,
    pub zec_usd: Option<f64>,
    pub price_time: &'a Option<String>,
    pub status: &'static str,
    pub status_text: String,
    pub latest: Option<serde_json::Value>,
    pub rounds: usize,
    /// Set when this fund's most recent submission was refused.
    pub last_refusal: Option<String>,
}

pub fn fund_view<'a>(f: &'a Fund, tip: u64, events: &[Event]) -> FundView<'a> {
    let obligations_zat: u64 = f.obligations.iter().map(|o| o.zat).sum();
    let latest = f.attestations.last();
    let last_refusal = events.iter().rev().find(|e| e.fund_id == f.id).filter(|e| !e.accepted).map(|e| e.summary.clone());
    let (status, status_text) = match latest {
        None => match &last_refusal {
            Some(r) => ("red", format!("Submission refused: {r}")),
            None => ("none", "No proof published yet".to_string()),
        },
        Some(s) => {
            let a = &s.attestation;
            let held = a.at_height - a.since_height;
            if !s.verdict.ok {
                ("red", "Latest proof failed verification".into())
            } else if a.threshold_zat < obligations_zat {
                ("amber", format!("Reserve {} ZEC is below obligations of {} ZEC", zec(a.threshold_zat), zec(obligations_zat)))
            } else if held < f.min_held_blocks {
                ("amber", format!("Reserve only held for {held} block{} (since block {}); the fund promises {}", if held == 1 { "" } else { "s" }, a.since_height, f.min_held_blocks))
            } else {
                ("green", format!("Reserves cover obligations, held since block {}", a.since_height))
            }
        }
    };
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
        name: &f.name,
        description: &f.description,
        obligations: &f.obligations,
        obligations_zat,
        min_held_blocks: f.min_held_blocks,
        zec_usd: f.zec_usd,
        price_time: &f.price_time,
        status,
        status_text,
        latest,
        rounds: f.attestations.len(),
        last_refusal,
    }
}

// ------------------------------------------------------------------ HTTP

fn json_resp(code: u16, v: &impl Serialize) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    tiny_http::Response::from_data(serde_json::to_vec(v).unwrap())
        .with_status_code(code)
        .with_header("Content-Type: application/json".parse::<tiny_http::Header>().unwrap())
        .with_header("Access-Control-Allow-Origin: *".parse::<tiny_http::Header>().unwrap())
}

fn handle(d: &DataDir, reg_id: &str, method: &str, url: &str, body: &str) -> Result<(u16, serde_json::Value)> {
    let path = url.split('?').next().unwrap_or(url);
    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
    let ledger = Ledger::load(&d.ledger())?;
    let tip = ledger.tip();
    let mut reg = Registry::load_or_new(d, reg_id)?;
    use serde_json::json;
    Ok(match (method, parts.as_slice()) {
        ("GET", ["api", "state"]) => {
            let funds: Vec<_> = reg.funds.values().map(|f| fund_view(f, tip, &reg.events)).collect();
            let mut events = reg.events.clone();
            events.reverse();
            (200, json!({ "registry": reg.id, "network": ledger.network, "pool": ledger.pool, "tip": tip,
                          "tip_hash": ledger.block(tip)?.hash, "funds": funds, "events": events }))
        }
        ("GET", ["api", "funds", id]) => match reg.funds.get(*id) {
            Some(f) => (200, serde_json::to_value(fund_view(f, tip, &reg.events))?),
            None => (404, json!({"error": "no such fund"})),
        },
        ("GET", ["api", "funds", id, "attestation"]) => match reg.funds.get(*id).and_then(|f| f.attestations.last()) {
            Some(s) => (200, serde_json::to_value(&s.attestation)?),
            None => (404, json!({"error": "no attestation"})),
        },
        ("POST", ["api", "funds", id, "verify"]) => match reg.funds.get(*id).and_then(|f| f.attestations.last()) {
            Some(s) => {
                let v = verify(&s.attestation, &ledger);
                (200, serde_json::to_value(v)?)
            }
            None => (404, json!({"error": "no attestation"})),
        },
        ("GET", ["api", "funds", id, "viewing-key"]) => {
            let p = d.public().join(format!("viewing-key-{id}.json"));
            if p.exists() { (200, read_json(&p)?) } else { (200, serde_json::Value::Null) }
        }
        ("GET", ["api", "roots", h]) => (200, serde_json::to_value(ledger.roots(h.parse()?)?)?),
        ("POST", ["api", "funds"]) => {
            let f: Fund = serde_json::from_str(body)?;
            if f.id.is_empty() || !f.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                bail!("fund id must be letters, digits and dashes");
            }
            let id = f.id.clone();
            reg.upsert_fund(f);
            reg.save(d)?;
            (200, json!({"ok": true, "id": id}))
        }
        ("POST", ["api", "attest"]) => {
            let a: Attestation = serde_json::from_str(body).map_err(|e| anyhow!("bad attestation: {e}"))?;
            let (accepted, reason, verdict) = reg.submit(a, &ledger)?;
            reg.save(d)?;
            (if accepted { 200 } else { 422 }, json!({"accepted": accepted, "reason": reason, "verdict": verdict}))
        }
        _ => (404, json!({"error": "not found"})),
    })
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
fn static_file(web: &Path, url_path: &str) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
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

pub fn serve(d: DataDir, reg_id: String, addr: &str, web: PathBuf) -> Result<()> {
    let server = tiny_http::Server::http(addr).map_err(|e| anyhow!("{e}"))?;
    eprintln!("Z-Solvency registry {reg_id} on http://{addr}");
    if !web.join("index.html").is_file() {
        eprintln!("warning: no web build at {} (npm --prefix web ci && npm --prefix web run build)", web.display());
    }
    // Warm the proving system's verifying key so the first Verify is quick.
    std::thread::spawn(|| {
        let _ = voting_circuits::delegation::warm_delegation_keys();
    });
    for mut rq in server.incoming_requests() {
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
        let mut body = String::new();
        let _ = rq.as_reader().take(8 << 20).read_to_string(&mut body);
        let t0 = std::time::Instant::now();
        let resp = match handle(&d, &reg_id, &method, &url, &body) {
            Ok((code, v)) => json_resp(code, &v),
            Err(e) => json_resp(400, &serde_json::json!({"error": e.to_string()})),
        };
        eprintln!("{method} {url} {:?}", t0.elapsed());
        let _ = rq.respond(resp);
    }
    Ok(())
}
