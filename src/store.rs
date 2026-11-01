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

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let s = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&s).with_context(|| format!("parsing {}", path.display()))
}

#[derive(Clone, Debug)]
pub struct DataDir(pub PathBuf);

impl DataDir {
    pub fn ledger(&self) -> PathBuf {
        self.0.join("chain/ledger.json")
    }
    pub fn wallet(&self, name: &str) -> PathBuf {
        self.0.join(format!("wallets/{name}.json"))
    }
    /// A fund's secret signing key (fund side, like its wallet).
    pub fn fund_key(&self, fund_id: &str) -> PathBuf {
        self.0.join(format!("funds/{fund_id}.key.json"))
    }
    pub fn registry(&self) -> PathBuf {
        self.0.join("registry/registry.json")
    }
    pub fn public(&self) -> PathBuf {
        self.0.join("public")
    }
}
