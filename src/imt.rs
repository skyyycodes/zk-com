//! Alternate-nullifier Indexed Merkle Tree over every Ironwood nullifier
//! revealed up to a height. Non-membership in it proves a note was unspent.
//!
//! Mirrors the K=2 punctured-range tree contract of `voting-circuits`
//! (sentinels every 2^249, sorted, deduplicated, odd count, Poseidon leaves),
//! but without the 32-leaf limit of the crate's test fixture.

use voting_circuits::delegation::{build_sentinel_list, ImtError, ImtProofData, ImtProvider, IMT_DEPTH};
use voting_crypto_deps::halo2_gadgets::poseidon::primitives::{self as poseidon, ConstantLength};
use voting_crypto_deps::pasta_curves::pallas;


fn h2(a: pallas::Base, b: pallas::Base) -> pallas::Base {
    poseidon::Hash::<_, poseidon::P128Pow5T3, ConstantLength<2>, 3, 2>::init().hash([a, b])
}
fn h3(a: pallas::Base, b: pallas::Base, c: pallas::Base) -> pallas::Base {
    poseidon::Hash::<_, poseidon::P128Pow5T3, ConstantLength<3>, 3, 2>::init().hash([a, b, c])
}

pub struct NullifierImt {
    leaves: Vec<[pallas::Base; 3]>,
    levels: Vec<Vec<pallas::Base>>,
    root: pallas::Base,
}

impl NullifierImt {
    pub fn new(real_nfs: &[pallas::Base]) -> Self {
        let mut nfs = build_sentinel_list();
        nfs.extend_from_slice(real_nfs);
        nfs.sort();
        nfs.dedup();
        if nfs.len() % 2 == 0 {
            let pad = std::iter::once(2u64)
                .chain(1u64..)
                .map(pallas::Base::from)
                .find(|c| nfs.binary_search(c).is_err())
                .unwrap();
            let at = nfs.binary_search(&pad).unwrap_err();
            nfs.insert(at, pad);
        }
        let leaves: Vec<[pallas::Base; 3]> =
            (0..(nfs.len() - 1) / 2).map(|i| [nfs[2 * i], nfs[2 * i + 1], nfs[2 * i + 2]]).collect();

        let mut empty = vec![h3(pallas::Base::zero(), pallas::Base::zero(), pallas::Base::zero())];
        for _ in 1..=IMT_DEPTH {
            let p = *empty.last().unwrap();
            empty.push(h2(p, p));
        }
        let mut levels = vec![leaves.iter().map(|b| h3(b[0], b[1], b[2])).collect::<Vec<_>>()];
        for l in 0..IMT_DEPTH {
            let prev = &levels[l];
            let next: Vec<_> = prev.chunks(2).map(|p| h2(p[0], *p.get(1).unwrap_or(&empty[l]))).collect();
            levels.push(next);
        }
        let root = levels[IMT_DEPTH].first().copied().unwrap_or(empty[IMT_DEPTH]);
        NullifierImt { leaves, levels, root }
    }

    fn empty(l: usize) -> pallas::Base {
        let mut e = h3(pallas::Base::zero(), pallas::Base::zero(), pallas::Base::zero());
        for _ in 0..l {
            e = h2(e, e);
        }
        e
    }
}

impl ImtProvider for NullifierImt {
    fn root(&self) -> pallas::Base {
        self.root
    }

    fn non_membership_proof(&self, nf: pallas::Base) -> Result<ImtProofData, ImtError> {
        let i = self.leaves.partition_point(|[lo, _, _]| *lo < nf);
        let k = i
            .checked_sub(1)
            .filter(|&k| {
                let [lo, mid, hi] = self.leaves[k];
                let off = nf - lo;
                off != pallas::Base::zero() && off < hi - lo && nf != mid
            })
            .ok_or_else(|| ImtError("nullifier is in the revealed set: the note was spent".into()))?;
        let mut path = [pallas::Base::zero(); IMT_DEPTH];
        let mut idx = k;
        for l in 0..IMT_DEPTH {
            path[l] = self.levels[l].get(idx ^ 1).copied().unwrap_or_else(|| Self::empty(l));
            idx >>= 1;
        }
        Ok(ImtProofData { root: self.root, nf_bounds: self.leaves[k], leaf_pos: k as u32, path })
    }
}
