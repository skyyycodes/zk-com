use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::path::{Path, PathBuf};

pub fn write_json<T: Serialize>(path: &Path, v: &T) -> Result<()> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(v)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Like `write_json`, for secrets (wallets, fund keys): readable by the owner only.
pub fn write_secret_json<T: Serialize>(path: &Path, v: &T) -> Result<()> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let tmp = path.with_extension("tmp");
    let _ = std::fs::remove_file(&tmp);
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    std::io::Write::write_all(&mut o.open(&tmp)?, serde_json::to_string_pretty(v)?.as_bytes())?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let s = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&s).with_context(|| format!("parsing {}", path.display()))
}

#[derive(Clone, Debug)]
pub struct DataDir(pub PathBuf);

impl DataDir {
    /// Chain data: testnet's binary file if present, else the devnet's JSON.
    pub fn ledger(&self) -> PathBuf {
        let bin = self.testnet_ledger();
        if bin.exists() { bin } else { self.devnet_ledger() }
    }
    pub fn devnet_ledger(&self) -> PathBuf {
        self.0.join("chain/ledger.json")
    }
    pub fn testnet_ledger(&self) -> PathBuf {
        self.0.join("chain/ledger.bin")
    }
    pub fn wallet(&self, name: &str) -> PathBuf {
        self.0.join(format!("wallets/{name}.json"))
    }
    /// The fund console's settings (network, registry, wallet, fund).
    pub fn console(&self) -> PathBuf {
        self.0.join("console.json")
    }
    /// Rounds the console proved but could not publish yet.
    pub fn pending(&self) -> PathBuf {
        self.0.join("rounds")
    }
    pub fn registry(&self) -> PathBuf {
        self.0.join("registry/registry.json")
    }
    /// The registry's own certificate signing key. Secret.
    pub fn registry_key(&self) -> PathBuf {
        self.0.join("registry/registry-key.json")
    }
    /// An auditor's or regulator's console settings and signing key. Secret.
    pub fn authority(&self) -> PathBuf {
        self.0.join("authority.json")
    }
    pub fn public(&self) -> PathBuf {
        self.0.join("public")
    }
}
