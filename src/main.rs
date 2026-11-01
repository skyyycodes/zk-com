use anyhow::{anyhow, bail, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use zsolv::attest::{prove, verify, Attestation, ProveRequest};
use zsolv::ledger::Ledger;
use zsolv::registry::{Fund, Obligation, Registry};
use zsolv::store::{read_json, write_json, DataDir};
use zsolv::util::{parse_zec, zec};
use zsolv::wallet::{make_reserve, transfer, Transfer, Wallet};

const DEFAULT_REGISTRY: &str = "zsolv-devnet-registry-1";

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
    Reserve {
        #[arg(long)]
        wallet: String,
        #[arg(long)]
        amount: String,
    },
    /// Rebuild the Ironwood tree root and nullifier-IMT root at a height from chain data.
    Roots {
        #[arg(long)]
        height: Option<u64>,
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
        #[arg(long, default_value = DEFAULT_REGISTRY)]
        registry: String,
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
        #[arg(long)]
        zec_usd: Option<f64>,
        #[arg(long)]
        price_time: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long, default_value = DEFAULT_REGISTRY)]
        registry: String,
    },
    /// Submit an attestation to a registry server.
    Submit {
        file: PathBuf,
        #[arg(long, default_value = "http://127.0.0.1:8080")]
        url: String,
    },
    /// Run the registry and reserve pages.
    Serve {
        #[arg(long, default_value = "127.0.0.1:8080")]
        addr: String,
        #[arg(long, default_value = DEFAULT_REGISTRY)]
        registry: String,
        /// The built web app to serve.
        #[arg(long, default_value = "web/dist")]
        web: PathBuf,
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
enum WalletCmd {
    New { name: String },
    Show { name: String },
}

fn load_wallet(d: &DataDir, name: &str) -> Result<Wallet> {
    read_json(&d.wallet(name)).map_err(|_| anyhow!("no wallet named {name} (create it with `zsolv wallet new {name}`)"))
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
                if d.ledger().exists() && !force {
                    bail!("chain data already exists at {} (use --force to reset)", d.ledger().display());
                }
                Ledger::new().save(&d.ledger())?;
                println!("devnet initialised at block 0");
            }
            DevnetCmd::Mine { blocks } => {
                let mut l = Ledger::load(&d.ledger())?;
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
        Cmd::Wallet { cmd } => match cmd {
            WalletCmd::New { name } => {
                if d.wallet(&name).exists() {
                    bail!("wallet {name} exists");
                }
                write_json(&d.wallet(&name), &Wallet::generate(&name))?;
                println!("created wallet {name}");
            }
            WalletCmd::Show { name } => {
                let l = Ledger::load(&d.ledger())?;
                print!("{}", load_wallet(&d, &name)?.summary(l.tip()));
            }
        },
        Cmd::Send { to, amount, from, label, memo, reserve } => {
            let mut l = Ledger::load(&d.ledger())?;
            let mut tw = load_wallet(&d, &to)?;
            let amount = parse_zec(&amount)?;
            let h = if let Some(f) = &from {
                if f == &to {
                    bail!("use `zsolv reserve` to move funds within a wallet");
                }
                let mut fw = load_wallet(&d, f)?;
                let label = if label == "anonymous donor" { f.clone() } else { label };
                let h = transfer(&mut l, Transfer { from: Some(&mut fw), from_label: label, to: &mut tw, amount, memo, reserve })?;
                write_json(&d.wallet(f), &fw)?;
                h
            } else {
                transfer(&mut l, Transfer { from: None, from_label: label, to: &mut tw, amount, memo, reserve })?
            };
            write_json(&d.wallet(&to), &tw)?;
            l.save(&d.ledger())?;
            println!("block {h}: {} ZEC → {to}{}", zec(amount), if reserve { " (reserve note)" } else { "" });
        }
        Cmd::Reserve { wallet, amount } => {
            let mut l = Ledger::load(&d.ledger())?;
            let mut w = load_wallet(&d, &wallet)?;
            let amount = parse_zec(&amount)?;
            let h = make_reserve(&mut l, &mut w, amount)?;
            write_json(&d.wallet(&wallet), &w)?;
            l.save(&d.ledger())?;
            println!("block {h}: reserve note of exactly {} ZEC created in {wallet}", zec(amount));
        }
        Cmd::Roots { height } => {
            let l = Ledger::load(&d.ledger())?;
            l.check_links()?;
            let r = l.roots(height.unwrap_or(l.tip()))?;
            println!("{}", serde_json::to_string_pretty(&r)?);
        }
        Cmd::Prove { wallet, fund_id, threshold, since, at, round, registry, out, cheat_claim_since } => {
            let l = Ledger::load(&d.ledger())?;
            let w = load_wallet(&d, &wallet)?;
            let log = |s: &str| eprintln!("{s}");
            let a = prove(ProveRequest {
                wallet: &w,
                ledger: &l,
                registry,
                fund_id: fund_id.clone(),
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
                "attestation written to {}: at least {} ZEC, held since block {}, checked at block {}",
                out.display(), zec(a.threshold_zat), a.since_height, a.at_height
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
                println!("\nVALID: {} holds at least {} ZEC, held since block {}, checked at block {} (tip {}).",
                    a.fund_id, zec(a.threshold_zat), a.since_height, a.at_height, v.tip);
            } else {
                println!("\nINVALID");
                std::process::exit(1);
            }
        }
        Cmd::Fund { id, name, description, obligations, min_held_blocks, zec_usd, price_time, url, registry } => {
            let obligations = obligations
                .iter()
                .map(|o| {
                    let (label, z) = o.rsplit_once('=').ok_or_else(|| anyhow!("obligation must be label=ZEC"))?;
                    Ok(Obligation { label: label.trim().into(), zat: parse_zec(z)? })
                })
                .collect::<Result<Vec<_>>>()?;
            let f = Fund { id: id.clone(), name, description, obligations, min_held_blocks, zec_usd, price_time, attestations: vec![] };
            if let Some(url) = url {
                let r = ureq::post(&format!("{}/api/funds", url.trim_end_matches('/'))).set("Content-Type", "application/json").send_string(&serde_json::to_string(&f)?)?;
                println!("{}", r.into_string()?);
            } else {
                let mut reg = Registry::load_or_new(&d, &registry)?;
                reg.upsert_fund(f);
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
        Cmd::Serve { addr, registry, web } => zsolv::registry::serve(d, registry, &addr, web)?,
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
