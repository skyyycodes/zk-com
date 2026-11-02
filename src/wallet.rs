//! A minimal wallet. It keeps the spending key and the plaintext of the
//! notes it holds. On the devnet it records notes as it receives them; on
//! testnet it finds them by trial-decrypting every Ironwood action (`scan`),
//! with the key derived from the same seed as the fund's everyday wallet.
//! This file is the fund's secret: it never leaves the fund's machine.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use voting_circuits::rand::rngs::OsRng;
use voting_circuits::rand::Rng;
use voting_crypto_deps::orchard::{
    keys::{FullViewingKey, PreparedIncomingViewingKey, Scope, SpendingKey},
    note::{ExtractedNoteCommitment, NoteVersion, Nullifier, RandomSeed, Rho},
    note_encryption::{CompactAction, IronwoodDomain},
    value::NoteValue,
    Address, Note,
};
use voting_crypto_deps::zcash_note_encryption::{try_compact_note_decryption, EphemeralKeyBytes};

use crate::ledger::Ledger;
use crate::util::{b32, zec};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WalletNote {
    pub cmx: String,
    pub value_zat: u64,
    pub recipient: String,
    pub rho: String,
    pub rseed: String,
    pub height: u64,
    pub position: u64,
    pub spent_height: Option<u64>,
    #[serde(default)]
    pub reserve: bool,
    #[serde(default)]
    pub memo: String,
    #[serde(default)]
    pub counterparty: String,
    /// Sent to the wallet's internal (change) address.
    #[serde(default)]
    pub internal: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HistoryEntry {
    pub height: u64,
    pub direction: String,
    pub value_zat: u64,
    pub counterparty: String,
    pub memo: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Wallet {
    pub name: String,
    pub spending_key: String,
    pub notes: Vec<WalletNote>,
    #[serde(default)]
    pub history: Vec<HistoryEntry>,
    /// Testnet: scan from this height (the wallet's birthday).
    #[serde(default)]
    pub birthday: u64,
}

impl Wallet {
    pub fn generate(name: &str) -> Self {
        let sk = voting_crypto_deps::random_spending_key(&mut OsRng);
        Wallet { name: name.into(), spending_key: hex::encode(sk.to_bytes()), notes: vec![], history: vec![], birthday: 0 }
    }

    /// A new wallet from 24 fresh backup words (BIP 39), returned once so the
    /// user can write them down; `from_mnemonic` restores it.
    pub fn generate_with_words(name: &str, birthday: u64) -> Result<(Self, String)> {
        let mut entropy = [0u8; 32];
        OsRng.fill_bytes(&mut entropy);
        let words = bip39::Mnemonic::from_entropy(&entropy).map_err(|e| anyhow!("{e}"))?.to_string();
        Ok((Self::from_mnemonic(name, &words, 0, birthday)?, words))
    }

    /// The wallet's address as a Unified Address with one Orchard receiver,
    /// which wallets route into Ironwood after NU6.3. The devnet uses the
    /// regtest prefix.
    pub fn unified_address(&self, network: &str) -> Result<String> {
        use zcash_address::unified::{self, Encoding};
        use zcash_protocol::consensus::NetworkType;
        let raw = self.address()?.to_raw_address_bytes();
        let ua = unified::Address::try_from_items(unified::Revision::R0, vec![unified::Uitem::Data(unified::Receiver::Orchard(raw))])
            .map_err(|e| anyhow!("{e}"))?;
        let net = if network == crate::ledger::TESTNET { NetworkType::Test } else { NetworkType::Regtest };
        Ok(ua.encode(&net))
    }

    /// The note marked as the reserve, spent or not.
    pub fn reserve_note(&self) -> Option<&WalletNote> {
        self.notes.iter().rev().find(|n| n.reserve)
    }

    /// Unspent value outside the reserve: what payouts may spend.
    pub fn operating_balance(&self, h: u64) -> u64 {
        self.unspent_at(h).filter(|n| !n.reserve).map(|n| n.value_zat).sum()
    }

    /// The account's Orchard-protocol key from a BIP-39 seed phrase (ZIP 32,
    /// coin type 1 = testnet). Orchard receivers receive into Ironwood after
    /// NU6.3, so this is the key behind the same seed in Zkool or Zingo.
    pub fn from_mnemonic(name: &str, phrase: &str, account: u32, birthday: u64) -> Result<Self> {
        let m = bip39::Mnemonic::parse(phrase.trim()).map_err(|e| anyhow!("bad seed phrase: {e}"))?;
        let account = voting_crypto_deps::zip32::AccountId::try_from(account).map_err(|_| anyhow!("bad account index"))?;
        let sk = SpendingKey::from_zip32_seed(&m.to_seed(""), 1, account).map_err(|e| anyhow!("deriving the key: {e:?}"))?;
        Ok(Wallet { name: name.into(), spending_key: hex::encode(sk.to_bytes()), notes: vec![], history: vec![], birthday })
    }

    /// Find this wallet's Ironwood notes in testnet chain data, and which are
    /// spent. Keeps reserve marks on notes found again. Returns notes found.
    pub fn scan(&mut self, ledger: &Ledger) -> Result<usize> {
        let fvk = self.fvk()?;
        let ivks = [
            (false, PreparedIncomingViewingKey::new(&fvk.to_ivk(Scope::External))),
            (true, PreparedIncomingViewingKey::new(&fvk.to_ivk(Scope::Internal))),
        ];
        let reserve: std::collections::HashSet<String> = self.notes.iter().filter(|n| n.reserve).map(|n| n.cmx.clone()).collect();
        let spent = ledger.nullifier_heights();
        let mut notes = vec![];
        for it in ledger.scan_from(self.birthday) {
            let (Some(nf), Some(cmx)) = (Option::from(Nullifier::from_bytes(it.nf)), Option::from(ExtractedNoteCommitment::from_bytes(it.cmx))) else {
                continue;
            };
            let act = CompactAction::from_parts(nf, cmx, EphemeralKeyBytes(it.out.epk), it.out.ct);
            let dom = IronwoodDomain::for_compact_action(&act);
            for (internal, ivk) in &ivks {
                if let Some((note, _)) = try_compact_note_decryption(&dom, ivk, &act) {
                    let mut wn = wallet_note(&note, it.height, it.position, false, "", "");
                    wn.internal = *internal;
                    wn.reserve = reserve.contains(&wn.cmx);
                    wn.spent_height = spent.get(&note.nullifier(&fvk).to_bytes()).copied();
                    notes.push(wn);
                    break;
                }
            }
        }
        self.notes = notes;
        Ok(self.notes.len())
    }

    /// Mark the unspent note of exactly `amount` (or the one with this cmx
    /// prefix) as the reserve note. For testnet, where the fund's everyday
    /// wallet made the note by sending exactly T to itself.
    pub fn mark_reserve(&mut self, amount: u64, cmx: Option<&str>) -> Result<&WalletNote> {
        let hits: Vec<usize> = (0..self.notes.len())
            .filter(|&i| {
                let n = &self.notes[i];
                n.spent_height.is_none() && n.value_zat == amount && cmx.is_none_or(|c| n.cmx.starts_with(c))
            })
            .collect();
        match hits.as_slice() {
            [i] => {
                for n in &mut self.notes {
                    n.reserve = false;
                }
                self.notes[*i].reserve = true;
                Ok(&self.notes[*i])
            }
            [] => Err(anyhow!("{} has no unspent note of exactly {} ZEC (send exactly that amount to itself, then `zsolv wallet scan`)", self.name, zec(amount))),
            _ => Err(anyhow!("{} has {} unspent notes of exactly {} ZEC: pick one with --cmx", self.name, hits.len(), zec(amount))),
        }
    }

    pub fn sk(&self) -> Result<SpendingKey> {
        Option::from(SpendingKey::from_bytes(b32(&self.spending_key)?)).ok_or_else(|| anyhow!("bad spending key"))
    }

    pub fn fvk(&self) -> Result<FullViewingKey> {
        Ok(FullViewingKey::from(&self.sk()?))
    }

    pub fn address(&self) -> Result<Address> {
        Ok(self.fvk()?.address_at(0u32, Scope::External))
    }

    pub fn unspent_at(&self, h: u64) -> impl Iterator<Item = &WalletNote> {
        self.notes.iter().filter(move |n| n.height <= h && n.spent_height.map_or(true, |s| s > h))
    }

    pub fn balance(&self, h: u64) -> u64 {
        self.unspent_at(h).map(|n| n.value_zat).sum()
    }

    pub fn summary(&self, tip: u64) -> String {
        let mut s = format!("wallet {} — balance {} ZEC at block {tip}\n", self.name, zec(self.balance(tip)));
        for n in &self.notes {
            s += &format!(
                "  {} {:>12} ZEC  block {:<4} pos {:<4} {}{}\n",
                crate::util::short(&n.cmx),
                zec(n.value_zat),
                n.height,
                n.position,
                if n.reserve { "[reserve] " } else { "" },
                match n.spent_height { Some(h) => format!("spent at {h}"), None => "unspent".into() }
            );
        }
        s
    }
}

impl WalletNote {
    pub fn note(&self) -> Result<Note> {
        let recipient = Option::from(Address::from_raw_address_bytes(
            &hex::decode(&self.recipient)?.try_into().map_err(|_| anyhow!("bad address"))?,
        ))
        .ok_or_else(|| anyhow!("bad address"))?;
        let rho = Option::from(Rho::from_bytes(&b32(&self.rho)?)).ok_or_else(|| anyhow!("bad rho"))?;
        let rseed = Option::from(RandomSeed::from_bytes(b32(&self.rseed)?, &rho)).ok_or_else(|| anyhow!("bad rseed"))?;
        Option::from(Note::from_parts(recipient, NoteValue::from_raw(self.value_zat), rho, rseed, NoteVersion::V3))
            .ok_or_else(|| anyhow!("bad note"))
    }
}

/// Make a fresh Ironwood (V3) note for `to`, using `rho` (the nullifier of the
/// note spent alongside it, as in a real Action).
fn new_note(to: Address, value: u64, rho: Rho) -> Note {
    voting_crypto_deps::new_note(to, NoteValue::from_raw(value), rho, NoteVersion::V3, &mut OsRng)
}

fn random_rho() -> Rho {
    let (_, fvk, dummy) = voting_crypto_deps::dummy_note(&mut OsRng, None, NoteVersion::V3);
    voting_crypto_deps::rho_from_nullifier(dummy.nullifier(&fvk))
}

fn wallet_note(note: &Note, height: u64, position: u64, reserve: bool, memo: &str, counterparty: &str) -> WalletNote {
    let cmx = ExtractedNoteCommitment::from(note.commitment()).to_bytes();
    WalletNote {
        cmx: hex::encode(cmx),
        value_zat: note.value().inner(),
        recipient: hex::encode(note.recipient().to_raw_address_bytes()),
        rho: hex::encode(note.rho().to_bytes()),
        rseed: hex::encode(note.rseed().as_bytes()),
        height,
        position,
        spent_height: None,
        reserve,
        memo: memo.into(),
        counterparty: counterparty.into(),
        internal: false,
    }
}

pub struct Transfer<'a> {
    /// None = funds arriving from outside the demo (a donor or the faucet).
    pub from: Option<&'a mut Wallet>,
    pub from_label: String,
    pub to: &'a mut Wallet,
    pub amount: u64,
    pub memo: String,
    pub reserve: bool,
}

/// Mine one block containing a shielded transfer. Spending selects the
/// sender's non-reserve notes first, so payouts never touch the reserve.
pub fn transfer(ledger: &mut Ledger, t: Transfer) -> Result<u64> {
    let height = ledger.tip() + 1;
    let mut nullifiers = vec![];
    let mut commitments = vec![];
    let mut position = ledger.commitment_count();
    let to_addr = t.to.address()?;
    let to_name = t.to.name.clone();

    let rho;
    let mut change_note = None;
    if let Some(from) = t.from {
        let fvk = from.fvk()?;
        let mut picked = vec![];
        let mut sum = 0u64;
        let mut candidates: Vec<usize> = (0..from.notes.len())
            .filter(|&i| from.notes[i].spent_height.is_none())
            .collect();
        candidates.sort_by_key(|&i| (from.notes[i].reserve, from.notes[i].height));
        for i in candidates {
            if sum >= t.amount {
                break;
            }
            sum += from.notes[i].value_zat;
            picked.push(i);
        }
        if sum < t.amount {
            return Err(anyhow!("{} has only {} ZEC unspent", from.name, zec(sum)));
        }
        let mut first_nf = None;
        for &i in &picked {
            let nf = from.notes[i].note()?.nullifier(&fvk);
            first_nf.get_or_insert(nf);
            nullifiers.push(hex::encode(nf.to_bytes()));
            from.notes[i].spent_height = Some(height);
        }
        rho = voting_crypto_deps::rho_from_nullifier(first_nf.unwrap());
        from.history.push(HistoryEntry {
            height,
            direction: "out".into(),
            value_zat: t.amount,
            counterparty: to_name.clone(),
            memo: t.memo.clone(),
        });
        if sum > t.amount {
            let back = from.address()?;
            let n = new_note(back, sum - t.amount, random_rho());
            change_note = Some((n, from));
        }
    } else {
        rho = random_rho();
    }

    let note = new_note(to_addr, t.amount, rho);
    let wn = wallet_note(&note, height, position, t.reserve, &t.memo, &t.from_label);
    commitments.push(wn.cmx.clone());
    position += 1;
    t.to.notes.push(wn);
    t.to.history.push(HistoryEntry {
        height,
        direction: "in".into(),
        value_zat: t.amount,
        counterparty: t.from_label.clone(),
        memo: t.memo.clone(),
    });
    if let Some((n, from)) = change_note {
        let wn = wallet_note(&n, height, position, false, "change", "self");
        commitments.push(wn.cmx.clone());
        from.notes.push(wn);
    }
    Ok(ledger.push_block(commitments, nullifiers))
}

/// Move `amount` from the wallet's operating notes into one reserve note of
/// exactly that value, sent to itself. This is the note the fund proves over.
pub fn make_reserve(ledger: &mut Ledger, w: &mut Wallet, amount: u64) -> Result<u64> {
    let height = ledger.tip() + 1;
    let fvk = w.fvk()?;
    let mut position = ledger.commitment_count();
    let mut idx: Vec<usize> = (0..w.notes.len()).filter(|&i| w.notes[i].spent_height.is_none() && !w.notes[i].reserve).collect();
    idx.sort_by_key(|&i| std::cmp::Reverse(w.notes[i].value_zat));
    let (mut sum, mut picked) = (0u64, vec![]);
    for i in idx {
        if sum >= amount {
            break;
        }
        sum += w.notes[i].value_zat;
        picked.push(i);
    }
    if sum < amount {
        return Err(anyhow!("{} has only {} ZEC in operating notes", w.name, zec(sum)));
    }
    let mut nullifiers = vec![];
    let mut first = None;
    for &i in &picked {
        let nf = w.notes[i].note()?.nullifier(&fvk);
        first.get_or_insert(nf);
        nullifiers.push(hex::encode(nf.to_bytes()));
        w.notes[i].spent_height = Some(height);
    }
    let addr = w.address()?;
    let mut commitments = vec![];
    let n = new_note(addr, amount, voting_crypto_deps::rho_from_nullifier(first.unwrap()));
    let wn = wallet_note(&n, height, position, true, "reserve", "self");
    commitments.push(wn.cmx.clone());
    position += 1;
    w.notes.push(wn);
    if sum > amount {
        let c = new_note(addr, sum - amount, random_rho());
        let wn = wallet_note(&c, height, position, false, "change", "self");
        commitments.push(wn.cmx.clone());
        w.notes.push(wn);
    }
    Ok(ledger.push_block(commitments, nullifiers))
}
