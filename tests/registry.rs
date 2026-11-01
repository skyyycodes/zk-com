//! Registry rules: a fund id is bound to the key that registered it, and only
//! that key can change the profile or publish rounds.
use zsolv::attest::{prove, round_message, ProveRequest};
use zsolv::fundkey::FundKey;
use zsolv::ledger::Ledger;
use zsolv::registry::{sign_profile, Obligation, Profile, Registry};
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

    // a second fund, signed with its own key, counting the same note: refused and logged
    let shadow = prove(ProveRequest {
        wallet: &fund, ledger: &l, registry: REG.into(), fund_id: "shadow".into(), fund_key: &shadow_key, round: 1,
        threshold_zat: 480 * COIN, since: Some(x), at: h, cheat_claim_since: None, log: &log,
    })
    .unwrap();
    let (ok, why, _) = reg.submit(shadow, &l).unwrap();
    assert!(!ok);
    assert!(why.contains("already counted by Mill Workers' Strike Fund"), "{why}");
    assert!(!reg.events.last().unwrap().accepted);
}
