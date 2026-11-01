//! A local Ironwood devnet: the public chain data a verifier needs (note
//! commitments and revealed nullifiers per block). It stands in for testnet
//! data fetched from lightwalletd; `zsolv roots` works the same on either.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use voting_crypto_deps::pasta_curves::pallas;

use crate::imt::NullifierImt;
use crate::tree::NoteTree;
use crate::util::{b32, hexf};
use voting_circuits::delegation::ImtProvider;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Block {
    pub height: u64,
    pub hash: String,
    pub prev_hash: String,
    /// Extracted note commitments (cmx) appended to the Ironwood tree.
    pub commitments: Vec<String>,
    /// Ironwood nullifiers revealed by spends in this block.
    pub nullifiers: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Ledger {
    pub network: String,
    pub pool: String,
    pub blocks: Vec<Block>,
}

#[derive(Serialize, Debug, Clone)]
pub struct Roots {
    pub height: u64,
    pub block_hash: String,
    pub nc_root: String,
    pub nf_imt_root: String,
    pub commitments: usize,
    pub nullifiers: usize,
}

fn block_hash(prev: &str, height: u64, cms: &[String], nfs: &[String]) -> String {
    let mut st = blake2b_simd::Params::new().hash_length(32).personal(b"ZSolvencyDevnetB").to_state();
    st.update(prev.as_bytes());
    st.update(&height.to_le_bytes());
    for c in cms {
        st.update(b"c");
        st.update(c.as_bytes());
    }
    for n in nfs {
        st.update(b"n");
        st.update(n.as_bytes());
    }
    hex::encode(st.finalize().as_bytes())
}

impl Ledger {
    pub fn new() -> Self {
        let mut l = Ledger { network: "zsolv-devnet".into(), pool: "ironwood".into(), blocks: vec![] };
        l.push_block(vec![], vec![]);
        l
    }

    pub fn load(path: &Path) -> Result<Self> {
        let s = std::fs::read_to_string(path)
            .with_context(|| format!("no chain data at {} (run `zsolv devnet init`)", path.display()))?;
        Ok(serde_json::from_str(&s)?)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        crate::store::write_json(path, self)
    }

    pub fn tip(&self) -> u64 {
        self.blocks.last().map(|b| b.height).unwrap_or(0)
    }

    pub fn push_block(&mut self, commitments: Vec<String>, nullifiers: Vec<String>) -> u64 {
        let height = self.blocks.len() as u64;
        let prev = self.blocks.last().map(|b| b.hash.clone()).unwrap_or_else(|| "00".repeat(32));
        let hash = block_hash(&prev, height, &commitments, &nullifiers);
        self.blocks.push(Block { height, hash, prev_hash: prev, commitments, nullifiers });
        height
    }

    pub fn block(&self, h: u64) -> Result<&Block> {
        self.blocks.get(h as usize).ok_or_else(|| anyhow!("height {h} is above the tip {}", self.tip()))
    }

    /// Checks every block hash links up; a verifier runs this before trusting the data.
    pub fn check_links(&self) -> Result<()> {
        let mut prev = "00".repeat(32);
        for (i, b) in self.blocks.iter().enumerate() {
            if b.height != i as u64 || b.prev_hash != prev || b.hash != block_hash(&prev, b.height, &b.commitments, &b.nullifiers) {
                return Err(anyhow!("chain data broken at height {i}"));
            }
            prev = b.hash.clone();
        }
        Ok(())
    }

    pub fn commitments_upto(&self, h: u64) -> Result<Vec<[u8; 32]>> {
        self.block(h)?;
        self.blocks[..=h as usize].iter().flat_map(|b| b.commitments.iter()).map(|c| b32(c)).collect()
    }

    pub fn nullifiers_upto(&self, h: u64) -> Result<Vec<pallas::Base>> {
        self.block(h)?;
        self.blocks[..=h as usize].iter().flat_map(|b| b.nullifiers.iter()).map(|n| hexf(n)).collect()
    }

    pub fn note_tree(&self, h: u64) -> Result<NoteTree> {
        NoteTree::build(&self.commitments_upto(h)?)
    }

    pub fn nullifier_imt(&self, h: u64) -> Result<NullifierImt> {
        Ok(NullifierImt::new(&self.nullifiers_upto(h)?))
    }

    pub fn roots(&self, h: u64) -> Result<Roots> {
        let tree = self.note_tree(h)?;
        let nfs = self.nullifiers_upto(h)?;
        let imt = NullifierImt::new(&nfs);
        Ok(Roots {
            height: h,
            block_hash: self.block(h)?.hash.clone(),
            nc_root: crate::util::fhex(&tree.root()),
            nf_imt_root: crate::util::fhex(&imt.root()),
            commitments: tree.len(),
            nullifiers: nfs.len(),
        })
    }
}
