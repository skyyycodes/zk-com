//! Tuesday's gate, plus the cheats, as one end-to-end test with real proofs.
use zsolv::attest::{prove, verify, ProveRequest};
use zsolv::fundkey::FundKey;
use zsolv::ledger::Ledger;
use zsolv::util::COIN;
use zsolv::wallet::{make_reserve, transfer, Transfer, Wallet};

fn give(l: &mut Ledger, to: &mut Wallet, zec: u64, from: &str) -> u64 {
    transfer(l, Transfer { from: None, from_label: from.into(), to, amount: zec * COIN, memo: "donation".into(), reserve: false }).unwrap()
}

#[test]
fn gate_and_cheats() {
    let log = |s: &str| eprintln!("{s}");
    let mut l = Ledger::new();
    let mut fund = Wallet::generate("mill");
    let mut lender = Wallet::generate("lender");
    let mut quick = Wallet::generate("quick");
    give(&mut l, &mut fund, 300, "donor A");
    give(&mut l, &mut fund, 250, "donor B");
    let x = make_reserve(&mut l, &mut fund, 480 * COIN).unwrap();
    give(&mut l, &mut lender, 480, "lender's own money");
    for _ in 0..3 { give(&mut l, &mut fund, 5, "donor C"); }
    // a payout from operating notes must not touch the reserve
    transfer(&mut l, Transfer { from: Some(&mut fund), from_label: "fund".into(), to: &mut lender, amount: 10 * COIN, memo: "strike pay".into(), reserve: false }).unwrap();
    // cheat 1: lender sends 480 to Quick Relief just before audit
    let hm1 = transfer(&mut l, Transfer { from: Some(&mut lender), from_label: "lender".into(), to: &mut quick, amount: 480 * COIN, memo: "loan".into(), reserve: true }).unwrap();
    let h = l.push_block(vec![], vec![]);

    let keys: Vec<FundKey> = ["mill", "quick", "shadow"].iter().map(|id| FundKey::generate(id)).collect();
    fn mk<'a>(w: &'a Wallet, l: &'a Ledger, log: &'a dyn Fn(&str), h: u64, k: &'a FundKey, since: Option<u64>, cheat: Option<u64>) -> ProveRequest<'a> { ProveRequest {
        wallet: w, ledger: l, registry: "test-registry".into(), fund_id: k.fund_id.clone(), fund_key: k, round: 1,
        threshold_zat: 480 * COIN, since, at: h, cheat_claim_since: cheat, log,
    } }
    let key = |id: &str| keys.iter().find(|k| k.fund_id == id).unwrap();
    let req = |w, f, s, c| mk(w, &l, &log, h, key(f), s, c);

    // gate: honest fund
    let a = prove(req(&fund, "mill", Some(x), None)).unwrap();
    let v = verify(&a, &l);
    for c in &v.checks { eprintln!("{} {} — {}", if c.ok { "ok  " } else { "FAIL" }, c.name, c.detail); }
    assert!(v.ok);
    assert_eq!(a.since.registry_nullifiers, a.at.registry_nullifiers);
    assert_eq!(a.at.van_opening.steps * zsolv::util::STEP_ZAT, 480 * COIN);

    // cheat 1a: borrowed coins cannot be proven since X
    let e = prove(req(&quick, "quick", Some(x), None)).unwrap_err();
    eprintln!("borrowed since X refused: {e}");
    // cheat 1b: honest about H-1, passes but says held since H-1
    let b = prove(req(&quick, "quick", None, None)).unwrap();
    assert_eq!(b.since_height, hm1);
    assert!(verify(&b, &l).ok);
    // cheat 1c: relabel since-height to X: verification must fail
    let c = prove(req(&quick, "quick", None, Some(x))).unwrap();
    let vc = verify(&c, &l);
    assert!(!vc.ok);
    eprintln!("relabelled since refused: {:?}", vc.checks.iter().filter(|c| !c.ok).map(|c| &c.detail).collect::<Vec<_>>());

    // tamper: claim a higher threshold in the message
    let mut t = a.clone();
    t.threshold_zat = 960 * COIN;
    t.message = zsolv::attest::round_message(&t);
    assert!(!verify(&t, &l).ok);

    // malformed fields are refused, never a panic (the registry runs this on anyone's input)
    let bad: Vec<(&str, Box<dyn Fn(&mut zsolv::attest::Attestation)>)> = vec![
        ("truncated proof", Box::new(|t| t.at.proof.truncate(t.at.proof.len() / 2))),
        ("empty proof", Box::new(|t| t.at.proof.clear())),
        ("non-hex proof", Box::new(|t| t.since.proof = "zz".repeat(10))),
        ("bad rk", Box::new(|t| t.at.rk = "ff".repeat(32))),
        ("four nullifiers", Box::new(|t| { t.at.registry_nullifiers.pop(); })),
        ("huge steps", Box::new(|t| t.at.van_opening.steps = u64::MAX)),
        ("short signature", Box::new(|t| t.at.signature.truncate(10))),
        ("height above tip", Box::new(|t| { t.at.height = 1 << 40; t.at_height = 1 << 40; })),
        ("non-hex fund key", Box::new(|t| t.fund_key = "not a key".into())),
        ("non-hex block hash", Box::new(|t| t.at_block_hash = "é".repeat(40))),
    ];
    for (what, f) in &bad {
        let mut t = a.clone();
        f(&mut t);
        t.message = zsolv::attest::round_message(&t);
        assert!(!verify(&t, &l).ok, "{what} must not verify");
    }

    // tamper: swap in another fund key without its signature
    let mut t = a.clone();
    t.fund_key = key("quick").public().unwrap();
    t.message = zsolv::attest::round_message(&t);
    assert!(!verify(&t, &l).ok);

    // second fund reusing the same note yields the same registry nullifiers
    let shadow = prove(req(&fund, "shadow", Some(x), None)).unwrap();
    assert!(verify(&shadow, &l).ok);
    let reused: Vec<_> = shadow.at.registry_nullifiers.iter().filter(|n| a.at.registry_nullifiers.contains(n)).collect();
    assert!(!reused.is_empty(), "registry must be able to spot the reused note");
    // only real note's nullifier is guaranteed equal (padding is fresh per attestation)
    assert_eq!(shadow.at.registry_nullifiers[0], a.at.registry_nullifiers[0]);
}
