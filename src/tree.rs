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
