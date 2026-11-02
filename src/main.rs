use anyhow::{anyhow, bail, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use zsolv::attest::{prove, verify, Attestation, ProveRequest};
use zsolv::fundkey::FundKey;
use zsolv::ledger::Ledger;
use zsolv::registry::{sign_profile, Obligation, Profile, Registry};
use zsolv::store::{read_json, write_json, write_secret_json, DataDir};
use zsolv::util::{parse_zec, zec};
use zsolv::wallet::{make_reserve, transfer, Transfer, Wallet};

const DEFAULT_REGISTRY: &str = "zsolv-devnet-registry-1";
const DEFAULT_SERVER: &str = "https://testnet.zec.rocks:443";

#[derive(Parser)]
#[command(name = "zsolv", about = "Z-Solvency: show the money, not the people")]
struct Cli {
    /// Data directory (chain data, wallets, registry state).
    #[arg(long, global = true, default_value = "data", env = "ZSOLV_DATA")]
    data: PathBuf,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Local Ironwood devnet (stands in for testnet chain data).
    Devnet {
        #[command(subcommand)]
        cmd: DevnetCmd,
    },
    /// Zcash testnet chain data from a lightwalletd server.
    Testnet {
        #[command(subcommand)]
        cmd: TestnetCmd,
    },
    /// Registry operator: who may register new funds.
    Registry {
        #[command(subcommand)]
        cmd: RegistryCmd,
    },
    /// Fund-side wallets (secret: they never leave the fund).
    Wallet {
        #[command(subcommand)]
        cmd: WalletCmd,
    },
    /// Send shielded ZEC on the devnet. Without --from, it arrives from outside (a donor).
    Send {
        #[arg(long)]
        to: String,
        #[arg(long)]
        amount: String,
        #[arg(long)]
        from: Option<String>,
        /// Who the sender is, as the recipient's wallet would record it.
        #[arg(long, default_value = "anonymous donor")]
        label: String,
        #[arg(long, default_value = "")]
        memo: String,
        /// Mark the received note as the recipient's reserve note.
        #[arg(long)]
        reserve: bool,
    },
    /// Move ZEC from operating notes into one reserve note of exactly AMOUNT.
    /// On testnet, marks the existing unspent note of exactly AMOUNT instead.
    Reserve {
        #[arg(long)]
        wallet: String,
        #[arg(long)]
        amount: String,
        /// Testnet: which note, by cmx prefix, when several match.
        #[arg(long)]
        cmx: Option<String>,
    },
    /// Rebuild the Ironwood tree root and nullifier-IMT root at a height from chain data.
    Roots {
        #[arg(long)]
        height: Option<u64>,
        /// Also compare the rebuilt tree root with the node's own Ironwood tree state.
        #[arg(long)]
        check: bool,
        #[arg(long, default_value = DEFAULT_SERVER, env = "ZSOLV_SERVER")]
        server: String,
    },
    /// Make an attestation: at least T ZEC, held since block X, checked at block H.
    Prove {
        #[arg(long)]
        wallet: String,
        #[arg(long)]
        fund_id: String,
        /// T in ZEC (a multiple of 0.125).
        #[arg(long)]
        threshold: String,
        /// X: the block the reserve must have been held since (default: earliest possible).
        #[arg(long)]
        since: Option<u64>,
        /// H: the block to check at (default: tip).
        #[arg(long)]
        at: Option<u64>,
        #[arg(long, default_value_t = 1)]
        round: u64,
        #[arg(long, default_value = DEFAULT_REGISTRY, env = "ZSOLV_REGISTRY")]
        registry: String,
        /// The fund's signing key (default: <data>/funds/<fund-id>.key.json).
        #[arg(long)]
        fund_key: Option<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
        /// Demo of a cheat: label the attestation as held since this block.
        #[arg(long, hide = true)]
        cheat_claim_since: Option<u64>,
    },
    /// Check an attestation yourself against chain data.
    Verify {
        file: PathBuf,
    },
    /// Register or update a fund's public profile (direct on the registry state, or --url).
    /// Creates the fund's signing key on first use; every later change and round needs it.
    Fund {
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "")]
        description: String,
        /// "label=ZEC", repeatable.
        #[arg(long = "obligation")]
        obligations: Vec<String>,
        #[arg(long, default_value_t = 0)]
        min_held_blocks: u64,
        /// Promise a new proof at least this often (0: no promise). Default: a week of 75 s blocks.
        #[arg(long, default_value_t = 8064)]
        max_proof_age_blocks: u64,
        #[arg(long)]
        zec_usd: Option<f64>,
        #[arg(long)]
        price_time: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long, default_value = DEFAULT_REGISTRY, env = "ZSOLV_REGISTRY")]
        registry: String,
        /// The fund's signing key (default: <data>/funds/<id>.key.json).
        #[arg(long)]
        fund_key: Option<PathBuf>,
    },
    /// Submit an attestation to a registry server.
    Submit {
        file: PathBuf,
        #[arg(long, default_value = "http://127.0.0.1:8080")]
        url: String,
    },
    /// Run the registry and reserve pages.
    Serve {
        #[arg(long, default_value = "127.0.0.1:8080", env = "ZSOLV_ADDR")]
        addr: String,
        #[arg(long, default_value = DEFAULT_REGISTRY, env = "ZSOLV_REGISTRY")]
        registry: String,
        /// The built web app to serve.
        #[arg(long, default_value = "web/dist")]
        web: PathBuf,
        /// Costly requests (submissions, Verify, root rebuilds) per minute per client; 0 = no limit.
        #[arg(long, default_value_t = 10, env = "ZSOLV_RATE_LIMIT")]
        rate_limit: u32,
        /// Root rebuilds at new heights per hour across all clients (each ~10-20 s on testnet); 0 = no limit.
        #[arg(long, default_value_t = 12, env = "ZSOLV_REBUILDS_PER_HOUR")]
        rebuilds_per_hour: u32,
        /// Behind your own reverse proxy: identify clients by X-Forwarded-For.
        #[arg(long, env = "ZSOLV_TRUST_PROXY")]
        trust_proxy: bool,
    },
    /// Demo: write what publishing this wallet's viewing key would reveal, for the contrast panel.
    ViewingKeyDemo {
        #[arg(long)]
        wallet: String,
        #[arg(long)]
        fund_id: String,
    },
}

#[derive(Subcommand)]
enum DevnetCmd {
    Init {
        #[arg(long)]
        force: bool,
    },
    Mine {
        #[arg(long, default_value_t = 1)]
        blocks: u64,
    },
    Status,
}

#[derive(Subcommand)]
enum TestnetCmd {
    /// Show what the server serves: chain, tip, branch, Ironwood tree size.
    Info {
        #[arg(long, default_value = DEFAULT_SERVER, env = "ZSOLV_SERVER")]
        server: String,
    },
    /// Fetch Ironwood chain data from activation (or the last synced block) into <data>/chain/ledger.bin.
    Sync {
        #[arg(long, default_value = DEFAULT_SERVER, env = "ZSOLV_SERVER")]
        server: String,
        /// Stop at this height (default: tip minus --confirmations).
        #[arg(long)]
        to: Option<u64>,
        #[arg(long, default_value_t = 10)]
        confirmations: u64,
        /// Drop this many blocks from the local tip first (after a reorg).
        #[arg(long)]
        rewind: Option<u64>,
    },
}

#[derive(Subcommand)]
enum RegistryCmd {
    /// Let this fund id register with this key. Registration is then closed to unlisted ids.
    Allow {
        #[arg(long)]
        id: String,
        /// The fund's public key (`zsolv fund` prints it when it creates the key).
        #[arg(long)]
        key: String,
        #[arg(long, default_value = DEFAULT_REGISTRY, env = "ZSOLV_REGISTRY")]
        registry: String,
    },
    /// Let anyone register an unused fund id again.
    Open {
        #[arg(long, default_value = DEFAULT_REGISTRY, env = "ZSOLV_REGISTRY")]
        registry: String,
    },
}

#[derive(Subcommand)]
enum WalletCmd {
    New { name: String },
    /// Testnet: derive the wallet from the fund's seed phrase (the one its everyday wallet uses).
    Import {
        name: String,
        /// BIP-39 seed phrase. Prefer the env var to keep it out of shell history.
        #[arg(long, env = "ZSOLV_MNEMONIC", hide_env_values = true)]
        mnemonic: String,
        #[arg(long, default_value_t = 0)]
        account: u32,
        /// Scan from this height (default: Ironwood activation).
        #[arg(long, default_value_t = zsolv::ledger::TESTNET_IRONWOOD_ACTIVATION)]
        birthday: u64,
    },
    /// Testnet: find the wallet's notes by trial decryption, and which are spent.
    Scan { name: String },
    Show { name: String },
}

fn load_wallet(d: &DataDir, name: &str) -> Result<Wallet> {
    read_json(&d.wallet(name)).map_err(|_| anyhow!("no wallet named {name} (create it with `zsolv wallet new {name}`)"))
}

fn load_fund_key(d: &DataDir, fund_id: &str, path: Option<PathBuf>) -> Result<FundKey> {
    let p = path.unwrap_or_else(|| d.fund_key(fund_id));
    read_json(&p).map_err(|_| anyhow!("no fund key at {} (register the fund with `zsolv fund --id {fund_id}` first)", p.display()))
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let d = DataDir(cli.data.clone());
    match cli.cmd {
        Cmd::Devnet { cmd } => match cmd {
            DevnetCmd::Init { force } => {
                if d.testnet_ledger().exists() {
                    bail!("{} holds testnet chain data: use another --data directory for the devnet", d.0.display());
                }
                if d.devnet_ledger().exists() && !force {
                    bail!("chain data already exists at {} (use --force to reset)", d.devnet_ledger().display());
                }
                Ledger::new().save(&d.devnet_ledger())?;
                println!("devnet initialised at block 0");
            }
            DevnetCmd::Mine { blocks } => {
                let mut l = Ledger::load(&d.ledger())?;
                if !l.is_devnet() {
                    bail!("this data directory holds testnet chain data");
                }
                for _ in 0..blocks {
                    l.push_block(vec![], vec![]);
                }
                l.save(&d.ledger())?;
                println!("tip is now block {}", l.tip());
            }
            DevnetCmd::Status => {
                let l = Ledger::load(&d.ledger())?;
                let r = l.roots(l.tip())?;
                println!("{} ({} pool) tip {} — {} note commitments, {} nullifiers", l.network, l.pool, l.tip(), r.commitments, r.nullifiers);
            }
        },
        Cmd::Testnet { cmd } => match cmd {
            TestnetCmd::Info { server } => {
                let mut c = zsolv::lightwalletd::Lightwalletd::connect(&server)?;
                let i = c.info()?;
                println!("{} {} ({}), chain {}, branch {}, tip {} (estimated {}), node {} {}, protocol {}",
                    i.vendor, i.version, server, i.chain_name, i.consensus_branch_id, i.block_height, i.estimated_height,
                    i.zcashd_build, i.zcashd_subversion, i.lightwallet_protocol_version);
                let t = c.tree_state(i.block_height)?;
                println!("tree state at {}: hash {}, ironwood tree {} hex chars", t.height, t.hash, t.ironwood_tree.len());
                let mut n = 0;
                c.blocks(i.block_height, i.block_height, |b| {
                    n += 1;
                    println!("compact block {}: {} txs with Ironwood data, ironwood tree size {:?}",
                        b.height, b.vtx.len(), b.chain_metadata.map(|m| m.ironwood_commitment_tree_size));
                    Ok(())
                })?;
                if n == 0 { bail!("server returned no compact block for the tip"); }
            }
            TestnetCmd::Sync { server, to, confirmations, rewind } => {
                let path = d.testnet_ledger();
                let mut l = if path.exists() { Ledger::load(&path)? } else { Ledger::new_testnet() };
                if let Some(r) = rewind {
                    l.truncate(l.tip().saturating_sub(r).max(l.start_height()))?;
                }
                let mut c = zsolv::lightwalletd::Lightwalletd::connect(&server)?;
                let server_tip = c.tip()?;
                let target = to.unwrap_or(server_tip.saturating_sub(confirmations));
                if !l.blocks.is_empty() {
                    // After a reorg: step back until our block matches the server's, then drop the rest.
                    let (mut h, mut step, mut differs) = (l.tip().min(server_tip), 1u64, false);
                    loop {
                        let mut theirs = None;
                        c.blocks(h, h, |b| {
                            theirs = Some(zsolv::lightwalletd::display_hash(&b.hash));
                            Ok(())
                        })?;
                        if theirs.as_deref() == Some(l.block(h)?.hash.as_str()) {
                            break;
                        }
                        differs = true;
                        if h == l.start_height() {
                            bail!("the local chain data does not match the server's even at block {h}: delete {} and sync again", path.display());
                        }
                        h = h.saturating_sub(step).max(l.start_height());
                        step *= 2;
                    }
                    if differs {
                        eprintln!("reorg: the server's chain differs above block {h}; dropping local blocks {}..={}", h + 1, l.tip());
                        l.truncate(h)?;
                    }
                }
                let mut next = if l.blocks.is_empty() { zsolv::ledger::TESTNET_IRONWOOD_ACTIVATION } else { l.tip() + 1 };
                let t0 = std::time::Instant::now();
                let mut unsaved = 0;
                while next <= target {
                    let end = (next + 9_999).min(target);
                    c.blocks(next, end, |b| l.push_compact(&b))?;
                    if l.tip() != end {
                        bail!("server stopped at block {} of {next}..={end}", l.tip());
                    }
                    eprintln!("block {end} of {target}: {} Ironwood actions ({:.0?})", l.commitment_count(), t0.elapsed());
                    next = end + 1;
                    unsaved += 1;
                    if unsaved == 5 {
                        l.save(&path)?;
                        unsaved = 0;
                    }
                }
                l.save(&path)?;
                println!("testnet chain data: blocks {}..={}, {} Ironwood actions, at {}", l.start_height(), l.tip(), l.commitment_count(), path.display());
            }
        },
        Cmd::Registry { cmd } => match cmd {
            RegistryCmd::Allow { id, key, registry } => {
                zsolv::fundkey::check_key(&key)?;
                let mut reg = Registry::load_or_new(&d, &registry)?;
                let n = {
                    let allow = reg.allowlist.get_or_insert_with(Default::default);
                    allow.insert(id.clone(), key);
                    allow.len()
                };
                reg.save(&d)?;
                println!("registry {registry}: fund {id} may register with that key; registration is closed to the {n} listed id(s)");
            }
            RegistryCmd::Open { registry } => {
                let mut reg = Registry::load_or_new(&d, &registry)?;
                reg.allowlist = None;
                reg.save(&d)?;
                println!("registry {registry}: anyone may register an unused fund id");
            }
        },
        Cmd::Wallet { cmd } => match cmd {
            WalletCmd::New { name } => {
                if d.wallet(&name).exists() {
                    bail!("wallet {name} exists");
                }
                write_secret_json(&d.wallet(&name), &Wallet::generate(&name))?;
                println!("created wallet {name}");
            }
            WalletCmd::Import { name, mnemonic, account, birthday } => {
                if d.wallet(&name).exists() {
                    bail!("wallet {name} exists");
                }
                write_secret_json(&d.wallet(&name), &Wallet::from_mnemonic(&name, &mnemonic, account, birthday)?)?;
                println!("imported wallet {name} (account {account}, scanning from block {birthday}); run `zsolv wallet scan {name}`");
            }
            WalletCmd::Scan { name } => {
                let l = Ledger::load(&d.ledger())?;
                if l.is_devnet() {
                    bail!("devnet wallets record their notes directly; scan is for testnet chain data");
                }
                let mut w = load_wallet(&d, &name)?;
                let t0 = std::time::Instant::now();
                let n = w.scan(&l)?;
                write_secret_json(&d.wallet(&name), &w)?;
                eprintln!("scanned blocks {}..={} in {:.1?}: {n} notes", w.birthday.max(l.start_height()), l.tip(), t0.elapsed());
                print!("{}", w.summary(l.tip()));
            }
            WalletCmd::Show { name } => {
                let l = Ledger::load(&d.ledger())?;
                print!("{}", load_wallet(&d, &name)?.summary(l.tip()));
            }
        },
        Cmd::Send { to, amount, from, label, memo, reserve } => {
            let mut l = Ledger::load(&d.ledger())?;
            if !l.is_devnet() {
                bail!("`zsolv send` is devnet only: on testnet, send from the fund's everyday wallet");
            }
            let mut tw = load_wallet(&d, &to)?;
            let amount = parse_zec(&amount)?;
            let h = if let Some(f) = &from {
                if f == &to {
                    bail!("use `zsolv reserve` to move funds within a wallet");
                }
                let mut fw = load_wallet(&d, f)?;
                let label = if label == "anonymous donor" { f.clone() } else { label };
                let h = transfer(&mut l, Transfer { from: Some(&mut fw), from_label: label, to: &mut tw, amount, memo, reserve })?;
                write_secret_json(&d.wallet(f), &fw)?;
                h
            } else {
                transfer(&mut l, Transfer { from: None, from_label: label, to: &mut tw, amount, memo, reserve })?
            };
            write_secret_json(&d.wallet(&to), &tw)?;
            l.save(&d.ledger())?;
            println!("block {h}: {} ZEC → {to}{}", zec(amount), if reserve { " (reserve note)" } else { "" });
        }
        Cmd::Reserve { wallet, amount, cmx } => {
            let mut l = Ledger::load(&d.ledger())?;
            let mut w = load_wallet(&d, &wallet)?;
            let amount = parse_zec(&amount)?;
            if !l.is_devnet() {
                let n = w.mark_reserve(amount, cmx.as_deref())?;
                println!("reserve note {} of exactly {} {}, received at block {}", zsolv::util::short(&n.cmx), zec(amount), l.unit(), n.height);
                write_secret_json(&d.wallet(&wallet), &w)?;
                return Ok(());
            }
            let h = make_reserve(&mut l, &mut w, amount)?;
            write_secret_json(&d.wallet(&wallet), &w)?;
            l.save(&d.ledger())?;
            println!("block {h}: reserve note of exactly {} ZEC created in {wallet}", zec(amount));
        }
        Cmd::Roots { height, check, server } => {
            let l = Ledger::load(&d.ledger())?;
            l.check_links()?;
            let r = l.roots(height.unwrap_or(l.tip()))?;
            println!("{}", serde_json::to_string_pretty(&r)?);
            if check {
                if l.is_devnet() {
                    bail!("--check compares with a testnet node; this is devnet chain data");
                }
                let t = zsolv::lightwalletd::Lightwalletd::connect(&server)?.tree_state(r.height)?;
                let (root, size) = zsolv::tree::frontier_root(&t.ironwood_tree)?;
                let root = zsolv::util::fhex(&root);
                println!("node at {}: block {}, Ironwood tree root {root}, {size} commitments", server, t.hash);
                if t.hash != r.block_hash {
                    bail!("block {} hash differs from the node's: {} vs {}", r.height, r.block_hash, t.hash);
                }
                if root != r.nc_root || size != r.commitments as u64 {
                    bail!("rebuilt Ironwood root does NOT match the node's");
                }
                println!("MATCH: the rebuilt Ironwood tree root equals the node's at block {}", r.height);
            }
        }
        Cmd::Prove { wallet, fund_id, threshold, since, at, round, registry, fund_key, out, cheat_claim_since } => {
            let l = Ledger::load(&d.ledger())?;
            let w = load_wallet(&d, &wallet)?;
            let fk = load_fund_key(&d, &fund_id, fund_key)?;
            let log = |s: &str| eprintln!("{s}");
            let a = prove(ProveRequest {
                wallet: &w,
                ledger: &l,
                registry,
                fund_id: fund_id.clone(),
                fund_key: &fk,
                round,
                threshold_zat: parse_zec(&threshold)?,
                since,
                at: at.unwrap_or(l.tip()),
                cheat_claim_since,
                log: &log,
            })?;
            let out = out.unwrap_or_else(|| PathBuf::from(format!("{fund_id}-round{round}.json")));
            write_json(&out, &a)?;
            println!(
                "attestation written to {}: at least {} {}, held since block {}, checked at block {}",
                out.display(), zec(a.threshold_zat), l.unit(), a.since_height, a.at_height
            );
        }
        Cmd::Verify { file } => {
            let a: Attestation = read_json(&file)?;
            let l = Ledger::load(&d.ledger())?;
            let v = verify(&a, &l);
            for c in &v.checks {
                println!("{} {:<52} {}", if c.ok { "✔" } else { "✘" }, c.name, c.detail);
            }
            if v.ok {
                println!("\nVALID: {} holds at least {} {}, held since block {}, checked at block {} (tip {}).",
                    a.fund_id, zec(a.threshold_zat), l.unit(), a.since_height, a.at_height, v.tip);
            } else {
                println!("\nINVALID");
                std::process::exit(1);
            }
        }
        Cmd::Fund { id, name, description, obligations, min_held_blocks, max_proof_age_blocks, zec_usd, price_time, url, registry, fund_key } => {
            let obligations = obligations
                .iter()
                .map(|o| {
                    let (label, z) = o.rsplit_once('=').ok_or_else(|| anyhow!("obligation must be label=ZEC"))?;
                    Ok(Obligation { label: label.trim().into(), zat: parse_zec(z)? })
                })
                .collect::<Result<Vec<_>>>()?;
            let key_path = fund_key.unwrap_or_else(|| d.fund_key(&id));
            let fk = if key_path.exists() {
                load_fund_key(&d, &id, Some(key_path))?
            } else {
                let k = FundKey::generate(&id);
                write_secret_json(&key_path, &k)?;
                eprintln!("created fund key {} at {} (secret: keep it with the wallet)", k.public()?, key_path.display());
                k
            };
            let p = Profile { id: id.clone(), name, description, obligations, min_held_blocks, max_proof_age_blocks, zec_usd, price_time };
            // Milliseconds since the epoch: increases across updates without local state.
            let seq = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis() as u64;
            let u = sign_profile(&registry, &p, &fk, seq)?;
            if let Some(url) = url {
                let r = ureq::post(&format!("{}/api/funds", url.trim_end_matches('/')))
                    .set("Content-Type", "application/json")
                    .send_string(&serde_json::to_string(&u)?);
                match r {
                    Ok(r) => println!("{}", r.into_string()?),
                    Err(ureq::Error::Status(_, r)) => bail!("registry refused the profile: {}", r.into_string()?),
                    Err(e) => return Err(e.into()),
                }
            } else {
                let mut reg = Registry::load_or_new(&d, &registry)?;
                reg.register(u)?;
                reg.save(&d)?;
                println!("fund {id} registered");
            }
        }
        Cmd::Submit { file, url } => {
            let body = std::fs::read_to_string(&file)?;
            let r = ureq::post(&format!("{}/api/attest", url.trim_end_matches('/')))
                .set("Content-Type", "application/json")
                .send_string(&body);
            let (code, text) = match r {
                Ok(r) => (r.status(), r.into_string()?),
                Err(ureq::Error::Status(c, r)) => (c, r.into_string()?),
                Err(e) => return Err(e.into()),
            };
            let v: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::json!({"raw": text}));
            if let Some(e) = v.get("error") {
                bail!("registry error: {e}");
            }
            println!("{} {}", if code == 200 { "ACCEPTED:" } else { "REFUSED:" }, v["reason"].as_str().unwrap_or(""));
        }
        Cmd::Serve { addr, registry, web, rate_limit, rebuilds_per_hour, trust_proxy } => {
            zsolv::registry::serve(d, registry, &addr, web, zsolv::registry::RateLimit { per_minute: rate_limit, rebuilds_per_hour, trust_proxy })?
        }
        Cmd::ViewingKeyDemo { wallet, fund_id } => {
            let w = load_wallet(&d, &wallet)?;
            let l = Ledger::load(&d.ledger())?;
            let v = serde_json::json!({
                "note": "What publishing this fund's viewing key would reveal, permanently. Generated from the demo wallet; Z-Solvency never publishes this.",
                "balance_zat": w.balance(l.tip()),
                "history": w.history,
            });
            let p = d.public().join(format!("viewing-key-{fund_id}.json"));
            write_json(&p, &v)?;
            println!("wrote {}", p.display());
        }
    }
    Ok(())
}
