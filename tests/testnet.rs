//! The testnet path end to end, on synthetic compact blocks shaped like
//! lightwalletd's: real V3 note encryption, a seed-phrase wallet that finds
//! its notes by trial decryption, the binary chain file, then prove + verify.
use voting_circuits::ff::{Field, PrimeField};
use voting_circuits::rand::rngs::OsRng;
use voting_crypto_deps::orchard::{
    keys::Scope,
    note::{ExtractedNoteCommitment, NoteVersion, Nullifier},
    note_encryption::{IronwoodDomain, IronwoodNoteEncryption},
    value::NoteValue,
    Address,
};
use voting_crypto_deps::pasta_curves::pallas;
use voting_crypto_deps::zcash_note_encryption::Domain;
use zsolv::attest::{prove, verify, ProveRequest};
use zsolv::fundkey::FundKey;
use zsolv::ledger::{Ledger, TESTNET_IRONWOOD_ACTIVATION};
use zsolv::lightwalletd::{ChainMetadata, CompactAction, CompactBlock, CompactTx};
use zsolv::util::COIN;
use zsolv::wallet::Wallet;

fn random_nf() -> Nullifier {
    loop {
        if let Some(nf) = Option::from(Nullifier::from_bytes(&pallas::Base::random(&mut OsRng).to_repr())) {
            return nf;
        }
    }
}

/// One action spending `nf` (or a dummy) and paying `value` to `to`.
fn action(to: Address, value: u64, nf: Option<Nullifier>) -> CompactAction {
    let nf = nf.unwrap_or_else(random_nf);
    let note = voting_crypto_deps::new_note(to, NoteValue::from_raw(value), voting_crypto_deps::rho_from_nullifier(nf), NoteVersion::V3, &mut OsRng);
    let enc = IronwoodNoteEncryption::new(None, note, [0u8; 512]);
    CompactAction {
        nullifier: nf.to_bytes().to_vec(),
        cmx: ExtractedNoteCommitment::from(note.commitment()).to_bytes().to_vec(),
        ephemeral_key: IronwoodDomain::epk_bytes(enc.epk()).0.to_vec(),
        ciphertext: enc.encrypt_note_plaintext()[..52].to_vec(),
    }
}

struct Chain {
    l: Ledger,
    size: u32,
}
impl Chain {
    fn block(&mut self, actions: Vec<CompactAction>) -> u64 {
        let height = if self.l.blocks.is_empty() { TESTNET_IRONWOOD_ACTIVATION } else { self.l.tip() + 1 };
        self.size += actions.len() as u32;
        let mut hash = [0u8; 32];
        hash[..8].copy_from_slice(&height.to_le_bytes());
        let prev = self.l.blocks.last().map(|b| { let mut v = hex::decode(&b.hash).unwrap(); v.reverse(); v }).unwrap_or(vec![0; 32]);
        let vtx = if actions.is_empty() { vec![] } else { vec![CompactTx { index: 1, txid: vec![1; 32], ironwood_actions: actions }] };
        self.l
            .push_compact(&CompactBlock { height, hash: hash.to_vec(), prev_hash: prev, time: 0, vtx, chain_metadata: Some(ChainMetadata { ironwood_commitment_tree_size: self.size }) })
            .unwrap();
        height
    }
}

#[test]
fn seed_wallet_scans_and_proves_on_testnet_shaped_data() {
    let phrase = bip39::Mnemonic::from_entropy(&[7u8; 32]).unwrap().to_string();
    let mut w = Wallet::from_mnemonic("mill", &phrase, 0, 0).unwrap();
    let fvk = w.fvk().unwrap();
    let ext = fvk.address_at(0u32, Scope::External);
    let int = fvk.address_at(0u32, Scope::Internal);
    let stranger = Wallet::generate("other").address().unwrap();

    let mut c = Chain { l: Ledger::new_testnet(), size: 0 };
    c.block(vec![]);
    c.block(vec![action(stranger, 3 * COIN, None), action(ext, 500 * COIN, None)]);
    c.block(vec![action(stranger, COIN, None)]);

    // the binary file round-trips
    let dir = std::env::temp_dir().join(format!("zsolv-testnet-{}", std::process::id()));
    let path = dir.join("chain/ledger.bin");
    c.l.save(&path).unwrap();
    let l = Ledger::load(&path).unwrap();
    l.check_links().unwrap();
    assert_eq!(l.commitment_count(), 3);
    assert_eq!(w.scan(&l).unwrap(), 1);
    assert!(w.mark_reserve(480 * COIN, None).is_err(), "no note of exactly 480 yet");

    // the everyday wallet sends exactly T to itself: spends the 500 into 480 + 20 change
    let nf500 = w.notes[0].note().unwrap().nullifier(&fvk);
    let mut c = Chain { size: l.commitment_count() as u32, l };
    let x = c.block(vec![action(ext, 480 * COIN, Some(nf500)), action(int, 20 * COIN, None)]);
    for _ in 0..3 {
        c.block(vec![action(stranger, COIN, None)]);
    }
    // a donor happens to send another 480
    c.block(vec![action(ext, 480 * COIN, None)]);
    let h = c.block(vec![action(stranger, COIN, None)]);

    // a block whose reported tree size disagrees is refused
    let prev = { let mut v = hex::decode(&c.l.blocks.last().unwrap().hash).unwrap(); v.reverse(); v };
    let bad = CompactBlock { height: h + 1, hash: vec![9; 32], prev_hash: prev, time: 0, vtx: vec![], chain_metadata: Some(ChainMetadata { ironwood_commitment_tree_size: 999 }) };
    assert!(c.l.clone().push_compact(&bad).is_err());
    let l = c.l;

    assert_eq!(w.scan(&l).unwrap(), 4);
    let spent: Vec<_> = w.notes.iter().filter(|n| n.spent_height.is_some()).map(|n| (n.value_zat, n.spent_height)).collect();
    assert_eq!(spent, vec![(500 * COIN, Some(x))]);
    assert!(w.notes.iter().any(|n| n.internal && n.value_zat == 20 * COIN));
    assert!(w.mark_reserve(480 * COIN, None).is_err(), "two unspent 480 notes: must pick one");
    let pick = w.notes.iter().find(|n| n.value_zat == 480 * COIN && n.height == x).unwrap().cmx[..12].to_string();
    w.mark_reserve(480 * COIN, Some(&pick)).unwrap();
    // a rescan keeps the mark
    w.scan(&l).unwrap();
    assert_eq!(w.notes.iter().filter(|n| n.reserve).count(), 1);

    let key = FundKey::generate("mill");
    let log = |_: &str| {};
    let a = prove(ProveRequest {
        wallet: &w, ledger: &l, registry: "zsolv-testnet-registry-1".into(), fund_id: "mill".into(), fund_key: &key, round: 1,
        threshold_zat: 480 * COIN, since: Some(x), at: h, cheat_claim_since: None, log: &log,
    })
    .unwrap();
    let v = verify(&a, &l);
    for ch in &v.checks { eprintln!("{} {} — {}", if ch.ok { "ok  " } else { "FAIL" }, ch.name, ch.detail); }
    assert!(v.ok);
    assert!(v.checks.iter().any(|c| c.detail == "at least 480 TAZ"), "testnet amounts are TAZ");

    // a reserve note on the wallet's internal (change) address proves too
    w.mark_reserve(20 * COIN, None).unwrap();
    assert!(w.notes.iter().find(|n| n.reserve).unwrap().internal);
    let key = FundKey::generate("change");
    let c = prove(ProveRequest {
        wallet: &w, ledger: &l, registry: "zsolv-testnet-registry-1".into(), fund_id: "change".into(), fund_key: &key, round: 1,
        threshold_zat: 20 * COIN, since: Some(x), at: h, cheat_claim_since: None, log: &log,
    })
    .unwrap();
    assert!(verify(&c, &l).ok);
    let _ = std::fs::remove_dir_all(dir);
}
