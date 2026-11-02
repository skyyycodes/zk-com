//! Ironwood note commitment tree (depth 32, Sinsemilla) rebuilt from public
//! chain data, so any verifier can recompute the anchor for a height.

use anyhow::{anyhow, Result};
use voting_crypto_deps::incrementalmerkletree::{Hashable, Level};
use voting_crypto_deps::orchard::{
    note::ExtractedNoteCommitment,
    tree::{MerkleHashOrchard, MerklePath},
};
use voting_crypto_deps::pasta_curves::pallas;
use voting_crypto_deps::CircuitValue as _;

pub const DEPTH: usize = 32;

pub struct NoteTree {
    levels: Vec<Vec<MerkleHashOrchard>>,
}

impl NoteTree {
    pub fn build(cmxs: &[[u8; 32]]) -> Result<Self> {
        let mut leaves = Vec::with_capacity(cmxs.len());
        for c in cmxs {
            let cmx = Option::from(ExtractedNoteCommitment::from_bytes(c))
                .ok_or_else(|| anyhow!("bad note commitment in chain data"))?;
            leaves.push(MerkleHashOrchard::from_cmx(&cmx));
        }
        let mut levels = vec![leaves];
        for l in 0..DEPTH {
            let prev = &levels[l];
            let empty = MerkleHashOrchard::empty_root(Level::from(l as u8));
            let next: Vec<_> = prev
                .chunks(2)
                .map(|p| MerkleHashOrchard::combine(Level::from(l as u8), &p[0], p.get(1).unwrap_or(&empty)))
                .collect();
            levels.push(next);
        }
        Ok(NoteTree { levels })
    }

    pub fn len(&self) -> usize {
        self.levels[0].len()
    }

    pub fn root(&self) -> pallas::Base {
        self.levels[DEPTH]
            .first()
            .copied()
            .unwrap_or_else(|| MerkleHashOrchard::empty_root(Level::from(DEPTH as u8)))
            .circuit_value()
    }

    pub fn path(&self, position: u64) -> Result<MerklePath> {
        if position as usize >= self.len() {
            return Err(anyhow!("note position {position} is not in the tree at this height"));
        }
        let mut idx = position as usize;
        let mut auth = [MerkleHashOrchard::empty_root(Level::from(0)); DEPTH];
        for l in 0..DEPTH {
            let sib = idx ^ 1;
            auth[l] = self.levels[l]
                .get(sib)
                .copied()
                .unwrap_or_else(|| MerkleHashOrchard::empty_root(Level::from(l as u8)));
            idx >>= 1;
        }
        Ok(MerklePath::from_parts(position as u32, auth))
    }
}

/// The root of a tree given as a node's tree state (zcashd/Zebra
/// `z_gettreestate`, lightwalletd `GetTreeState`): a legacy
/// `CommitmentTree` serialization of left, right and parents, each an
/// optional 32-byte node, parents prefixed by a CompactSize count.
struct Reader<'a>(&'a [u8]);
impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        if self.0.len() < n {
            return Err(anyhow!("tree state is truncated"));
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    fn node(&mut self) -> Result<Option<MerkleHashOrchard>> {
        match self.take(1)?[0] {
            0 => Ok(None),
            1 => {
                let n: [u8; 32] = self.take(32)?.try_into().unwrap();
                Option::from(MerkleHashOrchard::from_bytes(&n)).map(Some).ok_or_else(|| anyhow!("bad node in tree state"))
            }
            x => Err(anyhow!("bad option tag {x} in tree state")),
        }
    }
}

pub fn frontier_root(hex_state: &str) -> Result<(pallas::Base, u64)> {
    let b = hex::decode(hex_state.trim())?;
    let mut r = Reader(&b);
    let left = r.node()?;
    let right = r.node()?;
    let n = match r.take(1)?[0] {
        x @ 0..=0xfc => x as usize,
        0xfd => u16::from_le_bytes(r.take(2)?.try_into().unwrap()) as usize,
        _ => return Err(anyhow!("tree state has too many parents")),
    };
    let mut parents = vec![];
    for _ in 0..n {
        parents.push(r.node()?);
    }
    let empty = |l: usize| MerkleHashOrchard::empty_root(Level::from(l as u8));
    let Some(left) = left else {
        return Ok((empty(DEPTH).circuit_value(), 0));
    };
    // Size: left/right fill positions 0 and 1; parent i covers 2^(i+1) leaves.
    let mut size = 1 + right.is_some() as u64;
    let mut cur = MerkleHashOrchard::combine(Level::from(0), &left, &right.unwrap_or_else(|| empty(0)));
    for (l, p) in parents.iter().enumerate() {
        let lvl = l + 1;
        cur = match p {
            Some(p) => {
                size += 1u64 << lvl;
                MerkleHashOrchard::combine(Level::from(lvl as u8), p, &cur)
            }
            None => MerkleHashOrchard::combine(Level::from(lvl as u8), &cur, &empty(lvl)),
        };
    }
    for lvl in parents.len() + 1..DEPTH {
        cur = MerkleHashOrchard::combine(Level::from(lvl as u8), &cur, &empty(lvl));
    }
    Ok((cur.circuit_value(), size))
}
