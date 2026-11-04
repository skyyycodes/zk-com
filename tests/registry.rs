//! Registry rules: a fund id is bound to the key that registered it, and only
//! that key can change the profile or publish rounds.
use zsolv::attest::{prove, round_message, ProveRequest};
use zsolv::fundkey::FundKey;
use zsolv::ledger::Ledger;
use zsolv::registry::{fund_view, sign_profile, Obligation, Profile, Registry};
use zsolv::store::DataDir;
use zsolv::util::COIN;
use zsolv::wallet::{make_reserve, transfer, Transfer, Wallet};

const REG: &str = "test-registry";

fn profile(id: &str, name: &str) -> Profile {
    Profile {
        id: id.into(),
        name: name.into(),
        obligations: vec![Obligation { label: "Strike pay".into(), zat: 480 * COIN }],
        ..Default::default()
    }
}

#[test]
fn profile_changes_need_the_fund_key() {
    let mut reg = Registry::new(REG);
    let mill = FundKey::generate("mill");
    let impostor = FundKey::generate("mill");

    let first = sign_profile(REG, &profile("mill", "Mill Workers' Strike Fund"), &mill, 1).unwrap();
    reg.register(first.clone()).unwrap();
    assert_eq!(reg.funds["mill"].key, mill.public().unwrap());

    // another key cannot take over the id
    let e = reg.register(sign_profile(REG, &profile("mill", "Send to my address"), &impostor, 2).unwrap()).unwrap_err();
    assert!(e.to_string().contains("registered to another key"), "{e}");

    // edited profile text under the fund's real signature
    let mut forged = first.clone();
    forged.profile = forged.profile.replace("Mill Workers' Strike Fund", "Send to my address");
    forged.seq = 3;
    assert!(reg.register(forged).unwrap_err().to_string().contains("signature invalid"));

    // replaying an accepted update (or an older one)
    assert!(reg.register(first).unwrap_err().to_string().contains("not newer"));

    // signed for another registry
    let other = sign_profile("other-registry", &profile("mill", "x"), &mill, 4).unwrap();
    assert!(reg.register(other).unwrap_err().to_string().contains("signature invalid"));

    // the fund itself can still update
    reg.register(sign_profile(REG, &profile("mill", "Mill Workers' Strike Fund (wk 2)"), &mill, 5).unwrap()).unwrap();
    assert_eq!(reg.funds["mill"].profile.name, "Mill Workers' Strike Fund (wk 2)");
    assert_eq!(reg.funds["mill"].profile_seq, 5);

    // a key made for one fund id cannot sign another's profile
    assert!(sign_profile(REG, &profile("quick", "x"), &mill, 1).is_err());
}

#[test]
fn profiles_stay_within_bounds() {
    let mut reg = Registry::new(REG);
    let k = FundKey::generate("mill");
    let try_profile = |reg: &mut Registry, p: Profile, seq| reg.register(sign_profile(REG, &p, &k, seq).unwrap());

    let mut p = profile("mill", &"x".repeat(121));
    assert!(try_profile(&mut reg, p.clone(), 1).unwrap_err().to_string().contains("name"));
    p.name = "Mill".into();
    p.description = "y".repeat(2001);
    assert!(try_profile(&mut reg, p.clone(), 1).unwrap_err().to_string().contains("description"));
    p.description.clear();
    // obligations that would overflow the total, or exceed all the ZEC there is
    p.obligations = vec![Obligation { label: "a".into(), zat: u64::MAX }, Obligation { label: "b".into(), zat: 1 }];
    assert!(try_profile(&mut reg, p.clone(), 1).unwrap_err().to_string().contains("21 million"));
    p.obligations = vec![Obligation { label: "a".into(), zat: 21_000_001 * COIN }];
    assert!(try_profile(&mut reg, p.clone(), 1).is_err());
    p.obligations = (0..51).map(|i| Obligation { label: format!("o{i}"), zat: COIN }).collect();
    assert!(try_profile(&mut reg, p.clone(), 1).is_err());
    p.obligations = vec![Obligation { label: "Strike pay".into(), zat: 480 * COIN }];
    p.zec_usd = Some(-1.0);
    assert!(try_profile(&mut reg, p.clone(), 1).unwrap_err().to_string().contains("zec_usd"));
    p.zec_usd = Some(40.0);
    try_profile(&mut reg, p, 1).unwrap();
    // ids: charset and length
    let long = "m".repeat(65);
    let kl = FundKey::generate(&long);
    assert!(reg.register(sign_profile(REG, &profile(&long, "x"), &kl, 1).unwrap()).is_err());
    let kb = FundKey::generate("a b");
    assert!(reg.register(sign_profile(REG, &profile("a b", "x"), &kb, 1).unwrap()).is_err());
}

#[test]
fn closed_registration_admits_only_listed_keys() {
    let mut reg = Registry::new(REG);
    let mill = FundKey::generate("mill");
    let other = FundKey::generate("mill");
    reg.allowlist = Some([("mill".to_string(), mill.public().unwrap())].into());
    let e = reg.register(sign_profile(REG, &profile("mill", "Squatter"), &other, 1).unwrap()).unwrap_err();
    assert!(e.to_string().contains("by invitation"), "{e}");
    let spam = FundKey::generate("spam");
    assert!(reg.register(sign_profile(REG, &profile("spam", "Buy now"), &spam, 1).unwrap()).is_err());
    reg.register(sign_profile(REG, &profile("mill", "Mill Workers' Strike Fund"), &mill, 1).unwrap()).unwrap();
    // a listed fund still updates as usual
    reg.register(sign_profile(REG, &profile("mill", "Mill Workers' Strike Fund (wk 2)"), &mill, 2).unwrap()).unwrap();
}

#[test]
fn registry_state_must_match_the_registry_id() {
    let dir = std::env::temp_dir().join(format!("zsolv-regid-{}", std::process::id()));
    let d = DataDir(dir.clone());
    Registry::new(REG).save(&d).unwrap();
    assert!(Registry::load_or_new(&d, REG).is_ok());
    let e = Registry::load_or_new(&d, "another-registry").unwrap_err();
    assert!(e.to_string().contains("not another-registry"), "{e}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn only_the_fund_key_publishes_rounds() {
    let log = |_: &str| {};
    let mut l = Ledger::new();
    let mut fund = Wallet::generate("mill");
    let mut other = Wallet::generate("impostor");
    transfer(&mut l, Transfer { from: None, from_label: "donor".into(), to: &mut fund, amount: 500 * COIN, memo: String::new(), reserve: false }).unwrap();
    let x = make_reserve(&mut l, &mut fund, 480 * COIN).unwrap();
    transfer(&mut l, Transfer { from: None, from_label: "own money".into(), to: &mut other, amount: 480 * COIN, memo: String::new(), reserve: true }).unwrap();
    let h = l.push_block(vec![], vec![]);

    let mill_key = FundKey::generate("mill");
    let impostor_key = FundKey::generate("mill");
    let shadow_key = FundKey::generate("shadow");
    let mut reg = Registry::new(REG);
    reg.register(sign_profile(REG, &profile("mill", "Mill Workers' Strike Fund"), &mill_key, 1).unwrap()).unwrap();
    reg.register(sign_profile(REG, &profile("shadow", "Shadow Solidarity Fund"), &shadow_key, 1).unwrap()).unwrap();

    let req = |w: &str, k: &FundKey, round: u64, since: Option<u64>| {
        let w = if w == "mill" { &fund } else { &other };
        prove(ProveRequest {
            wallet: w, ledger: &l, registry: REG.into(), fund_id: "mill".into(), fund_key: k, round,
            threshold_zat: 480 * COIN, since, at: h, cheat_claim_since: None, log: &log,
        })
    };

    let r1 = req("mill", &mill_key, 1, Some(x)).unwrap();
    let (ok, why, _) = reg.submit(r1.clone(), &l).unwrap();
    assert!(ok, "{why}");
    let events = reg.events.len();

    // status rules: late (amber) after a cadence plus a seventh, red after two cadences
    let mut cadenced = profile("mill", "Mill Workers' Strike Fund");
    cadenced.max_proof_age_blocks = 100;
    reg.register(sign_profile(REG, &cadenced, &mill_key, 2).unwrap()).unwrap();
    let later = |blocks: u64| {
        let mut l2 = l.clone();
        for _ in 0..blocks {
            l2.push_block(vec![], vec![]);
        }
        l2
    };
    assert_eq!(fund_view(&reg.funds["mill"], &later(114), &reg.events).status, "green");
    let late = fund_view(&reg.funds["mill"], &later(115), &reg.events);
    assert_eq!(late.status, "amber");
    assert!(late.status_text.starts_with("Last proof is 2 hours old; this fund proves every 2 hours"), "{}", late.status_text);
    let gone = fund_view(&reg.funds["mill"], &later(201), &reg.events);
    assert_eq!(gone.status, "red");
    assert!(gone.status_text.starts_with("No proof for 4 hours"), "{}", gone.status_text);
    // a reorg that replaces block H: the proof no longer verifies
    let mut other = Ledger::new();
    while other.tip() < h {
        other.push_block(vec![], vec![]);
    }
    let reorged = fund_view(&reg.funds["mill"], &other, &reg.events);
    assert_eq!(reorged.status, "red");
    assert!(reorged.status_text.contains("no longer verifies"), "{}", reorged.status_text);

    // impostor with their own 480 ZEC note and their own key: a valid proof, wrong fund
    let hijack = req("other", &impostor_key, 2, None).unwrap();
    let (ok, why, _) = reg.submit(hijack.clone(), &l).unwrap();
    assert!(!ok);
    assert!(why.contains("not signed by the key registered"), "{why}");

    // same, but naming the mill's public key without its signature
    let mut named = hijack;
    named.fund_key = mill_key.public().unwrap();
    named.message = round_message(&named);
    let (ok, why, _) = reg.submit(named, &l).unwrap();
    assert!(!ok);
    assert!(why.contains("fund signature invalid"), "{why}");

    // replay of the fund's own published round
    let (ok, why, _) = reg.submit(r1, &l).unwrap();
    assert!(!ok);
    assert!(why.contains("not newer"), "{why}");

    // none of that reaches the fund's page
    assert_eq!(reg.events.len(), events);
    assert_eq!(reg.funds["mill"].attestations.len(), 1);

    // the fund's next round still goes through
    let (ok, why, _) = reg.submit(req("mill", &mill_key, 2, Some(x)).unwrap(), &l).unwrap();
    assert!(ok, "{why}");

    // a later round may not be checked at an earlier block
    let earlier = prove(ProveRequest {
        wallet: &fund, ledger: &l, registry: REG.into(), fund_id: "mill".into(), fund_key: &mill_key, round: 3,
        threshold_zat: 480 * COIN, since: Some(x), at: h - 1, cheat_claim_since: None, log: &log,
    })
    .unwrap();
    let (ok, why, _) = reg.submit(earlier, &l).unwrap();
    assert!(!ok);
    assert!(why.contains("before round 2"), "{why}");

    // a second fund, signed with its own key, counting the same note: refused and logged
    let shadow = prove(ProveRequest {
        wallet: &fund, ledger: &l, registry: REG.into(), fund_id: "shadow".into(), fund_key: &shadow_key, round: 1,
        threshold_zat: 480 * COIN, since: Some(x), at: h, cheat_claim_since: None, log: &log,
    })
    .unwrap();
    let (ok, why, _) = reg.submit(shadow, &l).unwrap();
    assert!(!ok);
    assert!(why.contains("already counted by another fund"), "{why}");
    // funds are unlisted: the refusal doesn't name the fund that owns the note
    assert!(!why.contains("Mill"), "{why}");
    assert!(!reg.events.last().unwrap().accepted);

    // the past-rounds strip: the fund's accepted rounds, and refusals of its own signed rounds
    let mill = fund_view(&reg.funds["mill"], &l, &reg.events);
    assert_eq!(mill.past.iter().map(|p| (p.accepted, p.round)).collect::<Vec<_>>(), vec![(true, Some(1)), (true, Some(2))]);
    let shadow = fund_view(&reg.funds["shadow"], &l, &reg.events);
    assert_eq!(shadow.past.len(), 1);
    assert!(!shadow.past[0].accepted && shadow.past[0].light == "red");
}

#[test]
fn fund_keys_come_from_the_wallet() {
    let w = Wallet::generate("mill");
    let sk = w.sk().unwrap();
    let pk = |k: FundKey| k.public().unwrap();
    assert_eq!(pk(FundKey::derive(&sk, "mill")), pk(FundKey::derive(&sk, "mill")));
    assert_ne!(pk(FundKey::derive(&sk, "mill")), pk(FundKey::derive(&sk, "shadow")));
    assert_ne!(pk(FundKey::derive(&sk, "mill")), pk(FundKey::derive(&Wallet::generate("other").sk().unwrap(), "mill")));
    // the backup words restore the wallet, so they restore the fund key
    let (w2, words) = Wallet::generate_with_words("fund", 0).unwrap();
    assert_eq!(words.split_whitespace().count(), 24);
    let restored = Wallet::from_mnemonic("fund", &words, 0, 0).unwrap();
    assert_eq!(pk(FundKey::derive(&w2.sk().unwrap(), "mill")), pk(FundKey::derive(&restored.sk().unwrap(), "mill")));
    // the registry binds and accepts it like any fund key
    let key = FundKey::derive(&sk, "mill");
    let mut reg = Registry::new(REG);
    reg.register(sign_profile(REG, &profile("mill", "Mill"), &key, 1).unwrap()).unwrap();
    assert_eq!(reg.funds["mill"].key, key.public().unwrap());
    // its address, for receiving into the fund's own wallet
    assert!(w.unified_address("testnet").unwrap().starts_with("utest1"));
    assert!(w.unified_address("zsolv-devnet").unwrap().starts_with("uregtest1"));
}

#[test]
fn funds_are_reached_only_by_their_link() {
    let mut reg = Registry::new(REG);
    let (mill, shadow) = (FundKey::generate("mill"), FundKey::generate("shadow"));
    reg.register(sign_profile(REG, &profile("mill", "Mill"), &mill, 1).unwrap()).unwrap();
    reg.register(sign_profile(REG, &profile("shadow", "Shadow"), &shadow, 1).unwrap()).unwrap();
    let link = reg.funds["mill"].link.clone();
    // 128 random bits, distinct per fund, and not the guessable id
    assert_eq!(link.len(), 32);
    assert!(link.chars().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(link, reg.funds["shadow"].link);
    assert_eq!(reg.by_link(&link).unwrap().profile.id, "mill");
    assert!(reg.by_link("mill").is_none());
    assert!(reg.by_link("").is_none());
    // a profile update keeps the link
    reg.register(sign_profile(REG, &profile("mill", "Mill (wk 2)"), &mill, 2).unwrap()).unwrap();
    assert_eq!(reg.funds["mill"].link, link);
    // funds saved before links existed get one, once
    reg.funds.get_mut("shadow").unwrap().link.clear();
    assert!(reg.ensure_links());
    assert!(!reg.ensure_links());
    assert_eq!(reg.funds["shadow"].link.len(), 32);
}
