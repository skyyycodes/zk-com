//! Public Ironwood chain data: per block, the note commitments appended to
//! the Ironwood tree and the nullifiers revealed. A verifier rebuilds roots
//! from it and never trusts copies.
//!
//! Two sources: a local devnet (`chain/ledger.json`), and Zcash testnet synced
//! from lightwalletd (`chain/ledger.bin`, see `zsolv testnet sync`). Testnet
//! also keeps each action's ephemeral key and compact ciphertext, so wallets
//! can find their notes by trial decryption.

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use voting_crypto_deps::pasta_curves::pallas;

use crate::imt::NullifierImt;
use crate::lightwalletd::{display_hash, CompactBlock};
use crate::tree::NoteTree;
use crate::util::{b32, hexf};
use voting_circuits::delegation::ImtProvider;

pub const DEVNET: &str = "zsolv-devnet";
pub const TESTNET: &str = "testnet";
/// NU6.3 (Ironwood) activation on Zcash testnet: the Ironwood tree is empty before it.
pub const TESTNET_IRONWOOD_ACTIVATION: u64 = 4_134_000;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Block {
    pub height: u64,
    pub hash: String,
    pub prev_hash: String,
    /// Ironwood commitments and nullifiers in the chain up to and including this block.
    pub cm_end: u64,
    pub nf_end: u64,
    /// The Ironwood tree size the server reported for this block, checked against `cm_end`.
    pub tree_size: Option<u64>,
}

/// Trial-decryption data for one action's output (testnet only).
#[derive(Clone, Copy, Debug)]
pub struct Output {
    pub epk: [u8; 32],
    pub ct: [u8; 52],
}

#[derive(Clone, Debug)]
pub struct Ledger {
    pub network: String,
    pub pool: String,
    pub blocks: Vec<Block>,
    cmx: Vec<[u8; 32]>,
    nfs: Vec<[u8; 32]>,
    /// Aligned with `cmx`; empty on the devnet.
    outputs: Vec<Output>,
    /// Where computed roots are cached (testnet: rebuilding takes seconds).
    cache: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Roots {
    pub height: u64,
    pub block_hash: String,
    pub nc_root: String,
    pub nf_imt_root: String,
    pub commitments: usize,
    pub nullifiers: usize,
}

/// One action as a wallet scans it.
pub struct ScanItem<'a> {
    pub height: u64,
    pub position: u64,
    pub nf: &'a [u8; 32],
    pub cmx: &'a [u8; 32],
    pub out: &'a Output,
}

fn devnet_hash(prev: &str, height: u64, cms: &[[u8; 32]], nfs: &[[u8; 32]]) -> String {
    let mut st = blake2b_simd::Params::new().hash_length(32).personal(b"ZSolvencyDevnetB").to_state();
    st.update(prev.as_bytes());
    st.update(&height.to_le_bytes());
    for c in cms {
        st.update(b"c");
        st.update(hex::encode(c).as_bytes());
    }
    for n in nfs {
        st.update(b"n");
        st.update(hex::encode(n).as_bytes());
    }
    hex::encode(st.finalize().as_bytes())
}

// The devnet's JSON shape, unchanged from before.
#[derive(Serialize, Deserialize)]
struct JsonBlock {
    height: u64,
    hash: String,
    prev_hash: String,
    commitments: Vec<String>,
    nullifiers: Vec<String>,
}
#[derive(Serialize, Deserialize)]
struct JsonLedger {
    network: String,
    pool: String,
    blocks: Vec<JsonBlock>,
}

const MAGIC: &[u8; 8] = b"ZSOLVLB1";

impl Ledger {
    fn empty(network: &str) -> Self {
        Ledger { network: network.into(), pool: "ironwood".into(), blocks: vec![], cmx: vec![], nfs: vec![], outputs: vec![], cache: None }
    }

    /// A fresh devnet with an empty genesis block.
    pub fn new() -> Self {
        let mut l = Self::empty(DEVNET);
        l.push_block(vec![], vec![]);
        l
    }

    /// Empty testnet chain data, to be filled by `push_compact` from activation.
    pub fn new_testnet() -> Self {
        Self::empty(TESTNET)
    }

    pub fn is_devnet(&self) -> bool {
        self.network == DEVNET
    }

    /// What amounts are called on this chain: testnet coins are TAZ.
    pub fn unit(&self) -> &'static str {
        if self.network == TESTNET { "TAZ" } else { "ZEC" }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let mut l = if path.extension().is_some_and(|e| e == "bin") {
            Self::load_bin(path)?
        } else {
            let s = std::fs::read_to_string(path)
                .with_context(|| format!("no chain data at {} (run `zsolv devnet init` or `zsolv testnet sync`)", path.display()))?;
            let j: JsonLedger = serde_json::from_str(&s)?;
            let mut l = Self::empty(&j.network);
            l.pool = j.pool;
            for b in j.blocks {
                for c in &b.commitments {
                    l.cmx.push(b32(c)?);
                }
                for n in &b.nullifiers {
                    l.nfs.push(b32(n)?);
                }
                l.blocks.push(Block { height: b.height, hash: b.hash, prev_hash: b.prev_hash, cm_end: l.cmx.len() as u64, nf_end: l.nfs.len() as u64, tree_size: None });
            }
            l
        };
        if !l.is_devnet() {
            l.cache = path.parent().map(|p| p.join("roots"));
        }
        Ok(l)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if path.extension().is_some_and(|e| e == "bin") {
            return self.save_bin(path);
        }
        let mut blocks = vec![];
        let (mut c0, mut n0) = (0usize, 0usize);
        for b in &self.blocks {
            let (c1, n1) = (b.cm_end as usize, b.nf_end as usize);
            blocks.push(JsonBlock {
                height: b.height,
                hash: b.hash.clone(),
                prev_hash: b.prev_hash.clone(),
                commitments: self.cmx[c0..c1].iter().map(hex::encode).collect(),
                nullifiers: self.nfs[n0..n1].iter().map(hex::encode).collect(),
            });
            (c0, n0) = (c1, n1);
        }
        crate::store::write_json(path, &JsonLedger { network: self.network.clone(), pool: self.pool.clone(), blocks })
    }

    // Binary layout: magic, network (u8 length + bytes), then per block:
    // height u64, hash [32], prev_hash [32], tree_size u64 (u64::MAX = none),
    // n_actions u32, and per action
    // nf [32], cmx [32], epk [32], ct [52]. Hashes are in display order.
    fn save_bin(&self, path: &Path) -> Result<()> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        if self.outputs.len() != self.cmx.len() || self.nfs.len() != self.cmx.len() {
            bail!("binary chain data needs one nullifier, commitment and output per action");
        }
        let tmp = path.with_extension("tmp");
        let mut w = BufWriter::new(std::fs::File::create(&tmp)?);
        w.write_all(MAGIC)?;
        w.write_all(&[self.network.len() as u8])?;
        w.write_all(self.network.as_bytes())?;
        let mut a0 = 0usize;
        for b in &self.blocks {
            let a1 = b.cm_end as usize;
            w.write_all(&b.height.to_le_bytes())?;
            w.write_all(&b32(&b.hash)?)?;
            w.write_all(&b32(&b.prev_hash)?)?;
            w.write_all(&b.tree_size.unwrap_or(u64::MAX).to_le_bytes())?;
            w.write_all(&((a1 - a0) as u32).to_le_bytes())?;
            for i in a0..a1 {
                w.write_all(&self.nfs[i])?;
                w.write_all(&self.cmx[i])?;
                w.write_all(&self.outputs[i].epk)?;
                w.write_all(&self.outputs[i].ct)?;
            }
            a0 = a1;
        }
        w.flush()?;
        drop(w);
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    fn load_bin(path: &Path) -> Result<Self> {
        let f = std::fs::File::open(path).with_context(|| format!("no chain data at {} (run `zsolv testnet sync`)", path.display()))?;
        let mut r = BufReader::with_capacity(1 << 20, f);
        let mut m = [0u8; 8];
        r.read_exact(&mut m)?;
        if &m != MAGIC {
            bail!("{} is not Z-Solvency chain data", path.display());
        }
        let mut n = [0u8; 1];
        r.read_exact(&mut n)?;
        let mut net = vec![0u8; n[0] as usize];
        r.read_exact(&mut net)?;
        let mut l = Self::empty(std::str::from_utf8(&net)?);
        let mut hdr = [0u8; 8 + 32 + 32 + 8 + 4];
        loop {
            match r.read_exact(&mut hdr) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e.into()),
            }
            let height = u64::from_le_bytes(hdr[0..8].try_into().unwrap());
            let ts = u64::from_le_bytes(hdr[72..80].try_into().unwrap());
            let n_actions = u32::from_le_bytes(hdr[80..84].try_into().unwrap());
            let mut a = [0u8; 32 * 3 + 52];
            for _ in 0..n_actions {
                r.read_exact(&mut a)?;
                l.nfs.push(a[0..32].try_into().unwrap());
                l.cmx.push(a[32..64].try_into().unwrap());
                l.outputs.push(Output { epk: a[64..96].try_into().unwrap(), ct: a[96..148].try_into().unwrap() });
            }
            l.blocks.push(Block {
                height,
                hash: hex::encode(&hdr[8..40]),
                prev_hash: hex::encode(&hdr[40..72]),
                cm_end: l.cmx.len() as u64,
                nf_end: l.nfs.len() as u64,
                tree_size: (ts != u64::MAX).then_some(ts),
            });
        }
        Ok(l)
    }

    pub fn start_height(&self) -> u64 {
        self.blocks.first().map(|b| b.height).unwrap_or(0)
    }

    pub fn tip(&self) -> u64 {
        self.blocks.last().map(|b| b.height).unwrap_or(0)
    }

    pub fn commitment_count(&self) -> u64 {
        self.cmx.len() as u64
    }

    /// Devnet: mine one block with these (hex) commitments and nullifiers.
    pub fn push_block(&mut self, commitments: Vec<String>, nullifiers: Vec<String>) -> u64 {
        let height = self.blocks.len() as u64;
        let prev = self.blocks.last().map(|b| b.hash.clone()).unwrap_or_else(|| "00".repeat(32));
        let cms: Vec<[u8; 32]> = commitments.iter().map(|c| b32(c).expect("commitment hex")).collect();
        let nfs: Vec<[u8; 32]> = nullifiers.iter().map(|n| b32(n).expect("nullifier hex")).collect();
        let hash = devnet_hash(&prev, height, &cms, &nfs);
        self.cmx.extend(cms);
        self.nfs.extend(nfs);
        self.blocks.push(Block { height, hash, prev_hash: prev, cm_end: self.cmx.len() as u64, nf_end: self.nfs.len() as u64, tree_size: None });
        height
    }

    /// Testnet: append a compact block from lightwalletd. It must extend the
    /// tip, and the Ironwood tree size the server reports must match.
    pub fn push_compact(&mut self, b: &CompactBlock) -> Result<()> {
        let hash = display_hash(&b.hash);
        let prev = display_hash(&b.prev_hash);
        if let Some(t) = self.blocks.last() {
            if b.height != t.height + 1 {
                bail!("expected block {}, got {}", t.height + 1, b.height);
            }
            if prev != t.hash {
                bail!("block {} does not extend {} (a reorg during sync): run sync again", b.height, t.height);
            }
        } else if b.height != TESTNET_IRONWOOD_ACTIVATION {
            bail!("testnet chain data must start at Ironwood activation, block {TESTNET_IRONWOOD_ACTIVATION}");
        }
        for tx in &b.vtx {
            for a in &tx.ironwood_actions {
                let f = |v: &[u8], n: &str| -> Result<[u8; 32]> {
                    v.try_into().map_err(|_| anyhow!("block {}: bad {n} length", b.height))
                };
                self.nfs.push(f(&a.nullifier, "nullifier")?);
                self.cmx.push(f(&a.cmx, "cmx")?);
                self.outputs.push(Output {
                    epk: f(&a.ephemeral_key, "ephemeral key")?,
                    ct: a.ciphertext.as_slice().try_into().map_err(|_| anyhow!("block {}: bad ciphertext length", b.height))?,
                });
            }
        }
        let tree_size = b.chain_metadata.as_ref().map(|m| m.ironwood_commitment_tree_size as u64);
        if let Some(ts) = tree_size {
            if ts != self.cmx.len() as u64 {
                bail!("block {}: server says the Ironwood tree has {ts} commitments, the blocks it sent add up to {}", b.height, self.cmx.len());
            }
        }
        self.blocks.push(Block { height: b.height, hash, prev_hash: prev, cm_end: self.cmx.len() as u64, nf_end: self.nfs.len() as u64, tree_size });
        Ok(())
    }

    /// Drop blocks above `h` (after a reorg).
    pub fn truncate(&mut self, h: u64) -> Result<()> {
        let keep = (self.idx(h)? + 1) as usize;
        self.blocks.truncate(keep);
        let b = self.blocks.last().unwrap();
        self.cmx.truncate(b.cm_end as usize);
        self.nfs.truncate(b.nf_end as usize);
        self.outputs.truncate(b.cm_end.min(self.outputs.len() as u64) as usize);
        Ok(())
    }

    fn idx(&self, h: u64) -> Result<u64> {
        let s = self.start_height();
        if h < s || h > self.tip() {
            bail!("height {h} is outside the chain data (blocks {s}..={})", self.tip());
        }
        Ok(h - s)
    }

    pub fn block(&self, h: u64) -> Result<&Block> {
        Ok(&self.blocks[self.idx(h)? as usize])
    }

    /// Checks the blocks link up (and on the devnet, that every hash is
    /// right); a verifier runs this before trusting the data.
    pub fn check_links(&self) -> Result<()> {
        let (mut c0, mut n0) = (0usize, 0usize);
        for (i, b) in self.blocks.iter().enumerate() {
            let h = self.start_height() + i as u64;
            if b.height != h || (i > 0 && b.prev_hash != self.blocks[i - 1].hash) {
                bail!("chain data broken at height {h}");
            }
            if self.is_devnet() {
                let prev = if i == 0 { "00".repeat(32) } else { self.blocks[i - 1].hash.clone() };
                let (c1, n1) = (b.cm_end as usize, b.nf_end as usize);
                if b.prev_hash != prev || b.hash != devnet_hash(&prev, h, &self.cmx[c0..c1], &self.nfs[n0..n1]) {
                    bail!("chain data broken at height {h}");
                }
                (c0, n0) = (c1, n1);
            } else if b.tree_size.is_some_and(|t| t != b.cm_end) {
                bail!("Ironwood tree size at block {h} does not match the commitments");
            }
        }
        Ok(())
    }

    pub fn commitments_upto(&self, h: u64) -> Result<&[[u8; 32]]> {
        Ok(&self.cmx[..self.block(h)?.cm_end as usize])
    }

    pub fn nullifiers_upto(&self, h: u64) -> Result<Vec<pallas::Base>> {
        self.nfs[..self.block(h)?.nf_end as usize].iter().map(|n| hexf(&hex::encode(n))).collect()
    }

    pub fn note_tree(&self, h: u64) -> Result<NoteTree> {
        NoteTree::build(self.commitments_upto(h)?)
    }

    pub fn nullifier_imt(&self, h: u64) -> Result<NullifierImt> {
        Ok(NullifierImt::new(&self.nullifiers_upto(h)?))
    }

    /// Every action from `from` on, for trial decryption (testnet only).
    pub fn scan_from(&self, from: u64) -> impl Iterator<Item = ScanItem<'_>> {
        let mut a0 = 0usize;
        self.blocks.iter().flat_map(move |b| {
            let a1 = b.cm_end as usize;
            let r = if b.height >= from && a1 <= self.outputs.len() { a0..a1 } else { a1..a1 };
            a0 = a1;
            r.map(move |i| ScanItem { height: b.height, position: i as u64, nf: &self.nfs[i], cmx: &self.cmx[i], out: &self.outputs[i] })
        })
    }

    /// Height at which each nullifier was revealed.
    pub fn nullifier_heights(&self) -> HashMap<[u8; 32], u64> {
        let mut m = HashMap::with_capacity(self.nfs.len());
        let mut n0 = 0usize;
        for b in &self.blocks {
            for nf in &self.nfs[n0..b.nf_end as usize] {
                m.entry(*nf).or_insert(b.height);
            }
            n0 = b.nf_end as usize;
        }
        m
    }

    fn roots_path(&self, h: u64) -> Result<Option<PathBuf>> {
        let hash = &self.block(h)?.hash;
        Ok(self.cache.as_ref().map(|d| d.join(format!("{}-{h}-{}.json", self.network, &hash[..16]))))
    }

    /// Whether roots at `h` are cheap: cached, or devnet-sized. Rebuilding
    /// at testnet size takes ~10-20 s.
    pub fn roots_are_cheap(&self, h: u64) -> bool {
        match self.roots_path(h) {
            Ok(None) => true,
            Ok(Some(p)) => p.is_file(),
            Err(_) => true, // out of range: refused without building anything
        }
    }

    pub fn roots(&self, h: u64) -> Result<Roots> {
        let hash = self.block(h)?.hash.clone();
        let cached = self.roots_path(h)?;
        if let Some(p) = &cached {
            if let Ok(r) = crate::store::read_json::<Roots>(p) {
                return Ok(r);
            }
        }
        let tree = self.note_tree(h)?;
        let nfs = self.nullifiers_upto(h)?;
        let imt = NullifierImt::new(&nfs);
        let r = Roots {
            height: h,
            block_hash: hash,
            nc_root: crate::util::fhex(&tree.root()),
            nf_imt_root: crate::util::fhex(&imt.root()),
            commitments: tree.len(),
            nullifiers: nfs.len(),
        };
        if let Some(p) = &cached {
            let _ = crate::store::write_json(p, &r);
        }
        Ok(r)
    }
}
