//! Small encoding helpers shared by every module.

use anyhow::{anyhow, Context, Result};
use voting_circuits::ff::PrimeField;
use voting_crypto_deps::pasta_curves::pallas;

/// One ZEC in zatoshi.
pub const COIN: u64 = 100_000_000;

/// Granularity of the value claim: the delegation circuit commits to
/// `floor(v_total / 12_500_000)` (0.125 ZEC steps) and rules out over-claiming.
pub const STEP_ZAT: u64 = 12_500_000;

/// All the ZEC there will ever be, in zatoshi.
pub const MAX_MONEY: u64 = 21_000_000 * COIN;

pub fn fhex(f: &pallas::Base) -> String {
    hex::encode(f.to_repr())
}

pub fn b32(s: &str) -> Result<[u8; 32]> {
    let v = hex::decode(s.trim()).with_context(|| format!("bad hex: {s}"))?;
    v.try_into().map_err(|_| anyhow!("expected 32 bytes: {s}"))
}

pub fn hexf(s: &str) -> Result<pallas::Base> {
    Option::from(pallas::Base::from_repr(b32(s)?)).ok_or_else(|| anyhow!("not a canonical field element: {s}"))
}

pub fn zec(zat: u64) -> String {
    let whole = zat / COIN;
    let frac = zat % COIN;
    if frac == 0 {
        format!("{whole}")
    } else {
        let s = format!("{whole}.{frac:08}");
        s.trim_end_matches('0').to_string()
    }
}

pub fn parse_zec(s: &str) -> Result<u64> {
    let s = s.trim();
    let (w, f) = s.split_once('.').unwrap_or((s, ""));
    if f.len() > 8 {
        return Err(anyhow!("too many decimals in {s}"));
    }
    let whole: u64 = if w.is_empty() { 0 } else { w.parse()? };
    let frac: u64 = if f.is_empty() { 0 } else { format!("{f:0<8}").parse()? };
    Ok(whole * COIN + frac)
}

/// Map a string label (registry id) to a Pallas base field element.
pub fn label_to_field(label: &str) -> pallas::Base {
    let mut h = blake2b_simd::Params::new()
        .hash_length(32)
        .personal(b"ZSolvency_Label_")
        .hash(label.as_bytes())
        .as_bytes()
        .to_vec();
    h[31] &= 0x3f; // < 2^254 < p
    pallas::Base::from_repr(h.try_into().unwrap()).unwrap()
}

pub fn short(h: &str) -> String {
    if h.len() > 16 { format!("{}…{}", &h[..8], &h[h.len() - 6..]) } else { h.to_string() }
}
