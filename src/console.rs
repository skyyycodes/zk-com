//! The fund console: a local web app for the fund's treasurer, served by
//! `zsolv console` on 127.0.0.1. It keeps the fund's wallet and derives the
//! fund key from it; only signed profiles and attestations leave the machine,
//! posted to the registry.
//!
//! The API answers only requests that carry this run's access token (printed
//! in the console link) and name this machine as their host, so no other web
//! page open in the browser can drive it.

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::attest::{prove, verify, Attestation, ProveRequest};
use crate::fundkey::FundKey;
use crate::ledger::{Ledger, DEVNET, TESTNET, TESTNET_IRONWOOD_ACTIVATION};
use crate::registry::{check_profile, guarded, sign_profile, static_file, Obligation, Profile};
use crate::store::{read_json, write_json, write_secret_json, DataDir};
use crate::util::{parse_zec, zec, STEP_ZAT};
use crate::wallet::{make_reserve, transfer, Transfer, Wallet};

pub const DEFAULT_SERVER: &str = "https://testnet.zec.rocks:443";

/// What `console.json` remembers between runs. No secrets: those live in the wallet file.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    /// Which chain data the console reads: `zsolv-devnet` or `testnet`.
    pub network: String,
    pub registry_url: String,
    pub registry_id: String,
    pub wallet: String,
    #[serde(default)]
    pub fund_id: Option<String>,
    /// The fund's share-link token, as the registry returned it on registration.
    #[serde(default)]
    pub link: Option<String>,
    /// The lightwalletd server testnet chain data comes from.
    #[serde(default = "default_server")]
    pub server: String,
    /// The last round this console published, so numbering continues while
    /// the registry is unreachable.
    #[serde(default)]
    pub last_round: u64,
}

fn default_server() -> String {
    DEFAULT_SERVER.into()
}

#[derive(Serialize, Clone, Debug)]
pub struct Step {
    pub label: String,
    /// pending, running, done or failed.
    pub state: &'static str,
    pub ms: Option<u64>,
    pub detail: Option<String>,
}

/// The prove-and-publish run in progress, polled by the page.
#[derive(Serialize, Clone, Debug, Default)]
pub struct Job {
    pub id: u64,
    pub running: bool,
    pub steps: Vec<Step>,
    pub result: Option<Value>,
    pub error: Option<String>,
}

pub struct Console {
    pub d: DataDir,
    job: Arc<Mutex<Job>>,
}

// ------------------------------------------------------------ registry client

/// An HTTP call to the registry: `Ok((status, body))`, or `Err` when it can't be reached.
fn http(method: &str, url: &str, body: Option<&str>) -> std::result::Result<(u16, Value), String> {
    let agent = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(5)).timeout(Duration::from_secs(240)).build();
    let req = agent.request(method, url).set("Content-Type", "application/json");
    let r = match body {
        Some(b) => req.send_string(b),
        None => req.call(),
    };
    let parse = |t: String| serde_json::from_str(&t).unwrap_or_else(|_| json!({ "error": t }));
    match r {
        Ok(resp) => Ok((resp.status(), parse(resp.into_string().map_err(|e| e.to_string())?))),
        Err(ureq::Error::Status(code, resp)) => Ok((code, parse(resp.into_string().unwrap_or_default()))),
        Err(e) => Err(e.to_string()),
    }
}

fn error_of(v: &Value) -> String {
    v.get("error").and_then(|e| e.as_str()).unwrap_or("unexpected reply from the registry").to_string()
}

/// The share link anyone can open to see and verify the fund's proof.
fn share_url(c: &Config) -> Option<String> {
    c.link.as_ref().map(|l| format!("{}/#/v/{l}", c.registry_url))
}

/// The fund's page data, read through its share link (the registry lists nothing).
fn registry_fund(c: &Config) -> std::result::Result<Option<Value>, String> {
    let Some(link) = &c.link else { return Ok(None) };
    match http("GET", &format!("{}/api/links/{link}", c.registry_url), None)? {
        (200, v) => Ok(Some(v)),
        (404, _) => Ok(None),
        (_, v) => Err(error_of(&v)),
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

// ------------------------------------------------------------------ requests

#[derive(Deserialize)]
pub struct SetupReq {
    /// create, import or existing.
    pub mode: String,
    /// devnet or testnet.
    pub network: String,
    pub registry_url: String,
    #[serde(default)]
    pub wallet: Option<String>,
    #[serde(default)]
    pub words: Option<String>,
    #[serde(default)]
    pub birthday: Option<u64>,
    #[serde(default)]
    pub server: Option<String>,
}

#[derive(Deserialize)]
pub struct RegisterReq {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub obligations: Vec<Obligation>,
    #[serde(default)]
    pub min_held_blocks: u64,
    #[serde(default)]
    pub max_proof_age_blocks: u64,
}

#[derive(Deserialize)]
pub struct MoveReq {
    /// ZEC, as typed ("12.5").
    pub amount: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub memo: String,
}

impl Console {
    pub fn new(d: DataDir) -> Self {
        Console { d, job: Arc::new(Mutex::new(Job::default())) }
    }

    fn config(&self) -> Result<Option<Config>> {
        let p = self.d.console();
        if p.exists() { Ok(Some(read_json(&p)?)) } else { Ok(None) }
    }

    fn need_config(&self) -> Result<Config> {
        self.config()?.ok_or_else(|| anyhow!("set up the console first"))
    }

    fn wallet(&self, c: &Config) -> Result<Wallet> {
        read_json(&self.d.wallet(&c.wallet))
    }

    fn save_wallet(&self, w: &Wallet) -> Result<()> {
        write_secret_json(&self.d.wallet(&w.name), w)
    }

    fn published(&self, round: u64) -> Result<()> {
        let mut c = self.need_config()?;
        c.last_round = c.last_round.max(round);
        write_json(&self.d.console(), &c)
    }

    fn need_idle(&self) -> Result<()> {
        if self.job.lock().unwrap_or_else(|e| e.into_inner()).running {
            bail!("a proof is running; wait for it to finish");
        }
        Ok(())
    }

    fn wallet_names(&self) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(self.d.0.join("wallets"))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.path().file_stem().map(|s| s.to_string_lossy().to_string()))
            .filter(|n| !n.ends_with(".tmp"))
            .collect();
        v.sort();
        v
    }

    fn pending(&self) -> Vec<Value> {
        let mut v: Vec<Value> = std::fs::read_dir(self.d.pending())
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let a: Attestation = read_json(&e.path()).ok()?;
                Some(json!({ "file": e.file_name().to_string_lossy(), "round": a.round, "at_height": a.at_height, "threshold_zat": a.threshold_zat }))
            })
            .collect();
        v.sort_by_key(|x| x["round"].as_u64());
        v
    }

    /// Everything the console screens show.
    pub fn state(&self) -> Result<Value> {
        let Some(c) = self.config()? else {
            return Ok(json!({ "configured": false, "wallets": self.wallet_names(), "devnet_here": self.d.devnet_ledger().exists() }));
        };
        let ledger = Ledger::load(&self.d.ledger()).ok();
        let tip = ledger.as_ref().map(|l| l.tip());
        let w = self.wallet(&c)?;
        let (balance, operating) = tip.map(|t| (w.balance(t), w.operating_balance(t))).unwrap_or((0, 0));
        let reserve = w.reserve_note().map(|n| json!({ "value_zat": n.value_zat, "height": n.height, "spent_height": n.spent_height, "cmx": n.cmx }));
        let (fund, registry_error) = match &c.fund_id {
            None => (Value::Null, None),
            Some(id) => {
                let key = FundKey::derive(&w.sk()?, id).public()?;
                let (view, err) = match registry_fund(&c) {
                    Ok(Some(v)) => (v, None),
                    Ok(None) if c.link.is_none() => (Value::Null, Some("save the fund page again to get its share link".to_string())),
                    Ok(None) => (Value::Null, Some("this fund isn't on the registry".to_string())),
                    Err(e) => (Value::Null, Some(format!("can't reach the registry: {e}"))),
                };
                (json!({ "id": id, "key": key, "page_url": share_url(&c), "registry": view }), err)
            }
        };
        Ok(json!({
            "configured": true,
            "network": c.network,
            "devnet": c.network == DEVNET,
            "unit": if c.network == TESTNET { "TAZ" } else { "ZEC" },
            "registry_url": c.registry_url,
            "registry_id": c.registry_id,
            "data_dir": self.d.0.display().to_string(),
            "tip": tip,
            "wallet": {
                "name": w.name,
                "address": w.unified_address(&c.network)?,
                "balance_zat": balance,
                "operating_zat": operating,
                "reserve": reserve,
                "notes": w.notes.len(),
            },
            "fund": fund,
            "registry_error": registry_error,
            "pending": self.pending(),
            "job": self.job.lock().unwrap_or_else(|e| e.into_inner()).clone(),
        }))
    }

    /// Step 1: pick the network and registry, and create, import or open the wallet.
    pub fn setup(&self, r: SetupReq) -> Result<Value> {
        if self.config()?.is_some() {
            bail!("this console is already set up (its settings are in {})", self.d.console().display());
        }
        let network = match r.network.as_str() {
            "devnet" => DEVNET,
            "testnet" => TESTNET,
            other => bail!("unknown network {other}"),
        };
        let registry_url = r.registry_url.trim().trim_end_matches('/').to_string();
        if !(registry_url.starts_with("http://") || registry_url.starts_with("https://")) {
            bail!("the registry address must start with http:// or https://");
        }
        let state = match http("GET", &format!("{registry_url}/api/state"), None) {
            Ok((200, v)) => v,
            Ok((_, v)) => bail!("that registry answered: {}", error_of(&v)),
            Err(e) => bail!("can't reach a registry at {registry_url}: {e}"),
        };
        let registry_id = state["registry"].as_str().ok_or_else(|| anyhow!("that address isn't a Z-Solvency registry"))?.to_string();
        if state["network"].as_str() != Some(network) {
            bail!("that registry runs on {}, not {network}", state["network"].as_str().unwrap_or("another chain"));
        }
        if network == DEVNET {
            let l = Ledger::load(&self.d.devnet_ledger())
                .map_err(|_| anyhow!("no devnet chain data in {}: run the console in the folder the devnet registry uses", self.d.0.display()))?;
            let (rt, rh) = (state["tip"].as_u64().unwrap_or(0), state["tip_hash"].as_str().unwrap_or(""));
            if l.block(rt).map(|b| b.hash != rh).unwrap_or(true) {
                bail!("that registry follows another devnet chain: run the console in the registry's own data folder");
            }
        }

        let name = r.wallet.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "fund".into());
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            bail!("wallet names are letters, digits, dashes and underscores");
        }
        let server = r.server.clone().unwrap_or_else(default_server);
        let birthday = |given: Option<u64>| -> u64 {
            given.unwrap_or_else(|| {
                if network == DEVNET {
                    0
                } else {
                    crate::lightwalletd::Lightwalletd::connect(&server)
                        .and_then(|mut c| c.tip())
                        .unwrap_or(TESTNET_IRONWOOD_ACTIVATION)
                }
            })
        };
        let mut words = None;
        match r.mode.as_str() {
            "existing" => self.wallet(&Config { network: network.into(), registry_url: registry_url.clone(), registry_id: registry_id.clone(), wallet: name.clone(), fund_id: None, link: None, server: server.clone(), last_round: 0 })
                .map_err(|_| anyhow!("no wallet named {name} in {}", self.d.0.display()))?,
            "create" | "import" => {
                if self.d.wallet(&name).exists() {
                    bail!("a wallet named {name} already exists here; open it instead");
                }
                let w = if r.mode == "create" {
                    let (w, ws) = Wallet::generate_with_words(&name, birthday(None))?;
                    words = Some(ws);
                    w
                } else {
                    let phrase = r.words.as_deref().unwrap_or("").trim();
                    if phrase.is_empty() {
                        bail!("enter the wallet's backup words");
                    }
                    Wallet::from_mnemonic(&name, phrase, 0, birthday(r.birthday.or(Some(TESTNET_IRONWOOD_ACTIVATION)).filter(|_| network == TESTNET)))?
                };
                self.save_wallet(&w)?;
                w
            }
            other => bail!("unknown setup mode {other}"),
        };
        // The registry lists no funds, so a fund this wallet registered before is
        // found again by registering the same id: its derived key matches, and
        // the registry returns the fund's link.
        let c = Config { network: network.into(), registry_url, registry_id, wallet: name, fund_id: None, link: None, server, last_round: 0 };
        write_json(&self.d.console(), &c)?;
        Ok(json!({ "ok": true, "words": words, "fund_id": Value::Null }))
    }

    /// Step 2: sign the profile with the fund key and publish the fund page.
    pub fn register(&self, r: RegisterReq) -> Result<Value> {
        self.need_idle()?;
        let mut c = self.need_config()?;
        if let Some(id) = &c.fund_id {
            if id != &r.id {
                bail!("this console already publishes for fund {id}");
            }
        }
        let w = self.wallet(&c)?;
        let p = Profile {
            id: r.id.trim().to_string(),
            name: r.name.trim().to_string(),
            description: r.description.trim().to_string(),
            obligations: r.obligations.into_iter().filter(|o| !o.label.trim().is_empty() || o.zat > 0).collect(),
            min_held_blocks: r.min_held_blocks,
            max_proof_age_blocks: r.max_proof_age_blocks,
            zec_usd: None,
            price_time: None,
        };
        check_profile(&p)?;
        let key = FundKey::derive(&w.sk()?, &p.id);
        let u = sign_profile(&c.registry_id, &p, &key, now_ms())?;
        let link = match http("POST", &format!("{}/api/funds", c.registry_url), Some(&serde_json::to_string(&u)?)) {
            Ok((200, v)) => v["link"].as_str().map(str::to_string),
            Ok((_, v)) => bail!("{}", error_of(&v)),
            Err(e) => bail!("can't reach the registry: {e}"),
        };
        c.fund_id = Some(p.id.clone());
        c.link = link.or(c.link);
        write_json(&self.d.console(), &c)?;
        Ok(json!({ "ok": true, "id": p.id, "page_url": share_url(&c) }))
    }

    /// Step 3: one note of exactly T, the obligations rounded up to 0.125.
    pub fn reserve(&self) -> Result<Value> {
        self.need_idle()?;
        let c = self.need_config()?;
        c.fund_id.as_ref().ok_or_else(|| anyhow!("register the fund first"))?;
        let view = registry_fund(&c).map_err(|e| anyhow!("can't reach the registry: {e}"))?.ok_or_else(|| anyhow!("the fund isn't on the registry"))?;
        let obligations = view["obligations_zat"].as_u64().unwrap_or(0);
        if obligations == 0 {
            bail!("add the fund's obligations first: the reserve covers them");
        }
        let t = obligations.div_ceil(STEP_ZAT) * STEP_ZAT;
        let unit = if c.network == TESTNET { "TAZ" } else { "ZEC" };
        let mut w = self.wallet(&c)?;
        if c.network == TESTNET {
            let l = self.sync_testnet(&c, &mut w, &|_| {})?;
            // Several notes of exactly T: the oldest has been held longest.
            let oldest = w.unspent_at(l.tip()).filter(|n| n.value_zat == t).min_by_key(|n| n.height).map(|n| n.cmx.clone());
            let Some(cmx) = oldest else {
                bail!(
                    "no note of exactly {} TAZ in this wallet yet. From your everyday wallet, send exactly {} TAZ to this fund's own address, wait for it to confirm, then press again.",
                    zec(t),
                    zec(t)
                );
            };
            let height = w.mark_reserve(t, Some(&cmx))?.height;
            self.save_wallet(&w)?;
            return Ok(json!({ "locked_zat": t, "height": height, "tip": l.tip() }));
        }
        let mut l = Ledger::load(&self.d.devnet_ledger())?;
        if let Some(r) = w.notes.iter().find(|n| n.reserve && n.spent_height.is_none()) {
            if r.value_zat == t {
                bail!("your reserve of exactly {} {unit} is already locked, since block {}", zec(t), r.height);
            }
        }
        // A reserve of another size (the obligations changed) folds into the new one.
        for n in w.notes.iter_mut().filter(|n| n.spent_height.is_none()) {
            n.reserve = false;
        }
        let have = w.balance(l.tip());
        if have < t {
            bail!("the fund holds {} {unit}; the reserve needs {} {unit}", zec(have), zec(t));
        }
        let h = make_reserve(&mut l, &mut w, t)?;
        l.save(&self.d.devnet_ledger())?;
        self.save_wallet(&w)?;
        Ok(json!({ "locked_zat": t, "height": h, "tip": l.tip() }))
    }

    fn sync_testnet(&self, c: &Config, w: &mut Wallet, log: &dyn Fn(&str)) -> Result<Ledger> {
        let l = crate::ledger::sync_testnet(&self.d.testnet_ledger(), &c.server, None, 10, None, log)?;
        w.scan(&l)?;
        Ok(l)
    }

    /// Step 4: start proving and publishing in the background; the page polls `job`.
    pub fn start_prove(&self) -> Result<Value> {
        let c = self.need_config()?;
        let fund_id = c.fund_id.clone().ok_or_else(|| anyhow!("register the fund first"))?;
        if self.wallet(&c)?.reserve_note().is_none() {
            bail!("lock the reserve first");
        }
        let mut job = self.job.lock().unwrap_or_else(|e| e.into_inner());
        if job.running {
            bail!("a proof is already running");
        }
        let host = c.registry_url.split("://").nth(1).unwrap_or(&c.registry_url).to_string();
        *job = Job {
            id: job.id + 1,
            running: true,
            steps: [
                "Chain data".to_string(),
                "Proof at block X".into(),
                "Proof at block H".into(),
                "Signed with spending key and fund key".into(),
                "Checked on this laptop".into(),
                format!("Published to {host}"),
            ]
            .into_iter()
            .map(|label| Step { label, state: "pending", ms: None, detail: None })
            .collect(),
            result: None,
            error: None,
        };
        let id = job.id;
        drop(job);
        let (d, shared) = (self.d.clone(), self.job.clone());
        std::thread::spawn(move || {
            let me = Console { d, job: shared.clone() };
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| me.run_prove(&c, &fund_id)));
            let mut j = shared.lock().unwrap_or_else(|e| e.into_inner());
            match outcome {
                Ok(Ok(v)) => j.result = Some(v),
                Ok(Err(e)) => j.error = Some(e.to_string()),
                Err(_) => j.error = Some("internal error while proving".into()),
            }
            if let Some(err) = j.error.clone() {
                if let Some(s) = j.steps.iter_mut().find(|s| s.state == "running" || s.state == "pending") {
                    s.state = "failed";
                    s.detail = Some(err);
                }
            }
            j.running = false;
        });
        Ok(json!({ "started": id }))
    }

    fn step(&self, i: usize, state: &'static str, ms: Option<u64>, detail: Option<String>) {
        let mut j = self.job.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = j.steps.get_mut(i) {
            s.state = state;
            if ms.is_some() {
                s.ms = ms;
            }
            if detail.is_some() {
                s.detail = detail;
            }
        }
    }

    fn label(&self, i: usize, label: String) {
        if let Some(s) = self.job.lock().unwrap_or_else(|e| e.into_inner()).steps.get_mut(i) {
            s.label = label;
        }
    }

    fn run_prove(&self, c: &Config, fund_id: &str) -> Result<Value> {
        // 1. Chain data to the tip, and the wallet's notes in it.
        self.step(0, "running", None, None);
        let t0 = Instant::now();
        let mut w = self.wallet(c)?;
        let l = if c.network == TESTNET {
            let l = self.sync_testnet(c, &mut w, &|m| self.step(0, "running", None, Some(m.to_string())))?;
            self.save_wallet(&w)?;
            l
        } else {
            let l = Ledger::load(&self.d.devnet_ledger())?;
            l.check_links()?;
            l
        };
        let h = l.tip();
        // Cheap checks before the slow root check: the scan may have found the reserve spent.
        let reserve = w.reserve_note().cloned().ok_or_else(|| anyhow!("lock the reserve first"))?;
        if let Some(s) = reserve.spent_height {
            bail!("your reserve note was spent at block {s}. Lock a new reserve; \"held since\" restarts.");
        }
        if c.network == TESTNET {
            self.step(0, "running", None, Some(format!("checking the rebuilt tree root at block {h} against the node")));
            crate::ledger::check_root_with_node(&l, h, &c.server).map_err(|e| anyhow!("chain data doesn't match the node ({e}). Not publishing."))?;
        }
        self.label(0, format!("Chain data to block {h}"));
        self.step(0, "done", Some(t0.elapsed().as_millis() as u64), Some(String::new()));
        let unit = l.unit();
        let t = reserve.value_zat / STEP_ZAT * STEP_ZAT;
        let x = reserve.height;
        self.label(1, format!("Proof at block {x}"));
        self.label(2, format!("Proof at block {h}"));

        let latest = registry_fund(c).ok().flatten();
        let pending_max = self.pending().iter().filter_map(|p| p["round"].as_u64()).max().unwrap_or(0);
        let round = latest.as_ref().and_then(|v| v["latest"]["round"].as_u64()).unwrap_or(0).max(pending_max).max(c.last_round) + 1;

        // 2-4. Both proofs, then the signatures; the prover's log marks the steps.
        let key = FundKey::derive(&w.sk()?, fund_id);
        let phase = Mutex::new((0usize, Instant::now()));
        let log = |m: &str| {
            let mut p = phase.lock().unwrap_or_else(|e| e.into_inner());
            if m.starts_with("proving") {
                self.step(1 + p.0, "running", None, None);
                p.1 = Instant::now();
            } else if m.trim_start().starts_with("done") {
                self.step(1 + p.0, "done", Some(p.1.elapsed().as_millis() as u64), None);
                p.0 += 1;
                p.1 = Instant::now();
            }
        };
        let a = prove(ProveRequest {
            wallet: &w,
            ledger: &l,
            registry: c.registry_id.clone(),
            fund_id: fund_id.into(),
            fund_key: &key,
            round,
            threshold_zat: t,
            since: Some(x),
            at: h,
            cheat_claim_since: None,
            log: &log,
        })
        .map_err(|e| {
            let m = e.to_string();
            if m.contains("at most") { anyhow!("combine your reserve into one note") } else { e }
        })?;
        let signed_ms = phase.lock().unwrap_or_else(|e| e.into_inner()).1.elapsed().as_millis() as u64;
        self.step(3, "done", Some(signed_ms), None);

        // 5. The same checks the registry and anyone else will run.
        self.step(4, "running", None, None);
        let t1 = Instant::now();
        let v = verify(&a, &l);
        let passed = v.checks.iter().filter(|c| c.ok).count();
        if !v.ok {
            let failed: Vec<_> = v.checks.iter().filter(|c| !c.ok).map(|c| c.detail.clone()).collect();
            bail!("{passed} of {} checks passed on this laptop: {}", v.checks.len(), failed.join("; "));
        }
        self.label(4, format!("Checked on this laptop, {passed} of {}", v.checks.len()));
        self.step(4, "done", Some(t1.elapsed().as_millis() as u64), None);

        // 6. Publish; keep the round to retry if the registry can't take it now.
        self.step(5, "running", None, None);
        let t2 = Instant::now();
        let page_url = share_url(c);
        let body = serde_json::to_string(&a)?;
        let claim = format!("at least {} {unit} · held since block {x} · checked at block {h}", zec(t));
        let reply = http("POST", &format!("{}/api/attest", c.registry_url), Some(&body));
        let saved = |why: String| -> Result<Value> {
            let file = self.d.pending().join(format!("{fund_id}-round{round}.json"));
            write_json(&file, &a)?;
            self.step(5, "failed", Some(t2.elapsed().as_millis() as u64), Some(format!("Saved round {round}. {why}")));
            Ok(json!({ "accepted": false, "saved": true, "round": round, "reason": why, "claim": claim, "page_url": page_url }))
        };
        match reply {
            Err(e) => saved(format!("Can't reach the registry ({e}); retry when it's back.")),
            Ok((429 | 503, v)) => saved(format!("The registry is busy ({}); retry shortly.", error_of(&v))),
            Ok((200, v)) => {
                self.published(round)?;
                self.step(5, "done", Some(t2.elapsed().as_millis() as u64), Some(format!("round {round}")));
                let light = registry_fund(c).ok().flatten().map(|f| f["status"].clone()).unwrap_or(Value::Null);
                Ok(json!({ "accepted": true, "round": round, "reason": v["reason"], "claim": claim, "page_url": page_url, "status": light }))
            }
            Ok((_, v)) => {
                let mut why = v["reason"].as_str().map(str::to_string).unwrap_or_else(|| error_of(&v));
                if why.contains("already counted by") {
                    why.push_str(". If you never shared your key, move your funds now.");
                }
                self.step(5, "failed", Some(t2.elapsed().as_millis() as u64), Some(format!("Refused: {why}")));
                Ok(json!({ "accepted": false, "round": round, "reason": why, "claim": claim, "page_url": page_url }))
            }
        }
    }

    /// Publish rounds saved while the registry was unreachable.
    pub fn retry(&self) -> Result<Value> {
        self.need_idle()?;
        let c = self.need_config()?;
        let mut out = vec![];
        for p in self.pending() {
            let file = self.d.pending().join(p["file"].as_str().unwrap_or_default());
            let body = std::fs::read_to_string(&file)?;
            match http("POST", &format!("{}/api/attest", c.registry_url), Some(&body)) {
                Err(e) => bail!("can't reach the registry: {e}"),
                Ok((429 | 503, v)) => bail!("the registry is busy: {}", error_of(&v)),
                Ok((code, v)) => {
                    if code == 200 {
                        self.published(p["round"].as_u64().unwrap_or(0))?;
                    }
                    std::fs::remove_file(&file)?;
                    out.push(json!({ "round": p["round"], "accepted": code == 200, "reason": v["reason"].as_str().map(str::to_string).unwrap_or_else(|| error_of(&v)) }));
                }
            }
        }
        Ok(json!({ "results": out }))
    }

    /// Devnet: a donation arrives from outside.
    pub fn receive(&self, r: MoveReq) -> Result<Value> {
        let (c, mut l, mut w) = self.devnet()?;
        let amount = parse_zec(&r.amount)?;
        if amount == 0 {
            bail!("enter an amount");
        }
        let label = if r.label.trim().is_empty() { "anonymous donor".to_string() } else { r.label.trim().to_string() };
        let h = transfer(&mut l, Transfer { from: None, from_label: label, to: &mut w, amount, memo: r.memo, reserve: false })?;
        self.save_devnet(&c, &l, &w)?;
        Ok(json!({ "height": h }))
    }

    /// Devnet: a payout from operating money. Refused if it would touch the reserve.
    pub fn pay(&self, r: MoveReq) -> Result<Value> {
        let (c, mut l, mut w) = self.devnet()?;
        let amount = parse_zec(&r.amount)?;
        if amount == 0 {
            bail!("enter an amount");
        }
        let operating = w.operating_balance(l.tip());
        if amount > operating {
            bail!("this payment would spend the reserve note: the fund has {} ZEC of operating money", zec(operating));
        }
        let payee_path = self.d.wallet("payees");
        let mut payees = if payee_path.exists() { read_json(&payee_path)? } else { Wallet::generate("payees") };
        let to = if r.label.trim().is_empty() { "members".to_string() } else { r.label.trim().to_string() };
        let h = transfer(&mut l, Transfer { from: Some(&mut w), from_label: c.wallet.clone(), to: &mut payees, amount, memo: r.memo, reserve: false })?;
        if let Some(entry) = w.history.last_mut() {
            entry.counterparty = to;
        }
        write_secret_json(&payee_path, &payees)?;
        self.save_devnet(&c, &l, &w)?;
        Ok(json!({ "height": h }))
    }

    /// Devnet: let time pass.
    pub fn mine(&self, blocks: u64) -> Result<Value> {
        let (c, mut l, w) = self.devnet()?;
        for _ in 0..blocks.clamp(1, 10_000) {
            l.push_block(vec![], vec![]);
        }
        self.save_devnet(&c, &l, &w)?;
        Ok(json!({ "tip": l.tip() }))
    }

    /// Testnet: fetch new blocks and find the wallet's notes.
    pub fn sync(&self) -> Result<Value> {
        self.need_idle()?;
        let c = self.need_config()?;
        if c.network != TESTNET {
            bail!("the devnet has nothing to sync");
        }
        let mut w = self.wallet(&c)?;
        let l = self.sync_testnet(&c, &mut w, &|_| {})?;
        self.save_wallet(&w)?;
        Ok(json!({ "tip": l.tip(), "notes": w.notes.len() }))
    }

    fn devnet(&self) -> Result<(Config, Ledger, Wallet)> {
        self.need_idle()?;
        let c = self.need_config()?;
        if c.network != DEVNET {
            bail!("only on the devnet: on testnet, use the fund's everyday wallet");
        }
        let l = Ledger::load(&self.d.devnet_ledger())?;
        let w = self.wallet(&c)?;
        Ok((c, l, w))
    }

    fn save_devnet(&self, _c: &Config, l: &Ledger, w: &Wallet) -> Result<()> {
        l.save(&self.d.devnet_ledger())?;
        self.save_wallet(w)
    }

    /// One API request (the path after `/api/console/`).
    pub fn handle(&self, method: &str, path: &str, body: &str) -> Result<(u16, Value)> {
        let parse = |b: &str| -> Result<Value> { if b.trim().is_empty() { Ok(json!({})) } else { Ok(serde_json::from_str(b)?) } };
        Ok(match (method, path) {
            ("GET", "state") => (200, self.state()?),
            ("GET", "job") => (200, serde_json::to_value(self.job.lock().unwrap_or_else(|e| e.into_inner()).clone())?),
            ("POST", "setup") => (200, self.setup(serde_json::from_str(body)?)?),
            ("POST", "register") => (200, self.register(serde_json::from_str(body)?)?),
            ("POST", "reserve") => (200, self.reserve()?),
            ("POST", "prove") => (200, self.start_prove()?),
            ("POST", "retry") => (200, self.retry()?),
            ("POST", "receive") => (200, self.receive(serde_json::from_str(body)?)?),
            ("POST", "pay") => (200, self.pay(serde_json::from_str(body)?)?),
            ("POST", "mine") => (200, self.mine(parse(body)?["blocks"].as_u64().unwrap_or(1))?),
            ("POST", "sync") => (200, self.sync()?),
            _ => (404, json!({ "error": "not found" })),
        })
    }
}

// ---------------------------------------------------------------- HTTP

fn resp(code: u16, v: &Value) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    // No CORS header: only the console's own page may read these.
    tiny_http::Response::from_data(serde_json::to_vec(v).unwrap())
        .with_status_code(code)
        .with_header("Content-Type: application/json".parse::<tiny_http::Header>().unwrap())
        .with_header("Cache-Control: no-store".parse::<tiny_http::Header>().unwrap())
}

fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn header<'a>(rq: &'a tiny_http::Request, name: &'static str) -> Option<&'a str> {
    rq.headers().iter().find(|h| h.field.equiv(name)).map(|h| h.value.as_str())
}

/// Serve the console on this machine only, until stopped.
pub fn serve(d: DataDir, addr: &str, web: PathBuf, open: bool) -> Result<()> {
    if !(addr.starts_with("127.0.0.1:") || addr.starts_with("localhost:") || addr.starts_with("[::1]:")) {
        bail!("the console holds the fund's keys, so it only listens on this machine (127.0.0.1:<port>)");
    }
    let server = tiny_http::Server::http(addr).map_err(|e| anyhow!("{e}"))?;
    let port = server.server_addr().to_ip().map(|a| a.port()).ok_or_else(|| anyhow!("no port"))?;
    let token = {
        use voting_circuits::rand::{rngs::OsRng, Rng};
        let mut b = [0u8; 32];
        OsRng.fill_bytes(&mut b);
        hex::encode(b)
    };
    let hosts = [format!("127.0.0.1:{port}"), format!("localhost:{port}"), format!("[::1]:{port}")];
    let origins: Vec<String> = hosts.iter().map(|h| format!("http://{h}")).collect();
    let url = format!("http://127.0.0.1:{port}/#token={token}");
    println!("Fund console: {url}");
    println!("Wallet and settings stay in {}. The link carries this run's access token; don't share it.", d.0.display());
    if !web.join("index.html").is_file() {
        eprintln!("warning: no web build at {} (npm --prefix web ci && npm --prefix web run build)", web.display());
    }
    if open {
        let cmd = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        let _ = std::process::Command::new(cmd).arg(&url).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn();
    }
    let console = Console::new(d);
    for mut rq in server.incoming_requests() {
        let method = rq.method().as_str().to_string();
        let url = rq.url().to_string();
        let path = url.split('?').next().unwrap_or("/").to_string();
        // A page on another site that resolves its name to 127.0.0.1 still sends its own Host.
        if !header(&rq, "Host").is_some_and(|h| hosts.iter().any(|x| x == h)) {
            let _ = rq.respond(resp(403, &json!({ "error": "the console answers only on this machine's own address" })));
            continue;
        }
        if !path.starts_with("/api/") {
            let r = if method == "GET" || method == "HEAD" { static_file(&web, &path) } else { resp(405, &json!({ "error": "method not allowed" })) };
            let _ = rq.respond(r);
            continue;
        }
        if path == "/api/mode" {
            let _ = rq.respond(resp(200, &json!({ "mode": "console" })));
            continue;
        }
        if !header(&rq, "X-Console-Token").is_some_and(|t| same(t.as_bytes(), token.as_bytes())) {
            let _ = rq.respond(resp(401, &json!({ "error": "open the console from the link `zsolv console` printed" })));
            continue;
        }
        if header(&rq, "Origin").is_some_and(|o| !origins.iter().any(|x| x == o)) {
            let _ = rq.respond(resp(403, &json!({ "error": "requests from other sites are refused" })));
            continue;
        }
        let mut body = String::new();
        let _ = rq.as_reader().take(1 << 20).read_to_string(&mut body);
        let sub = path.trim_start_matches("/api/console/").to_string();
        let t0 = Instant::now();
        let (code, v) = guarded(|| console.handle(&method, &sub, &body));
        if sub != "job" && sub != "state" {
            eprintln!("{method} {path} {code} {:?}", t0.elapsed());
        }
        let _ = rq.respond(resp(code, &v));
    }
    Ok(())
}
