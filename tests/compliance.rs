//! Certificates: the registry certifies every accepted round; approved
//! authorities certify rounds a fund asks them to, after checking the proof
//! themselves. Certificates stay private to the fund.
use zsolv::attest::{prove, verify, ProveRequest};
use zsolv::compliance::{
    check_certificate, decline_message, fund_read_message, inbox_message, issue, request_message, unix_now, DeclineBody, FundReadBody,
    IssuerKey, RequestBody, Signed,
};
use zsolv::fundkey::FundKey;
use zsolv::ledger::Ledger;
use zsolv::registry::{fund_view, sign_profile, IssueBody, Obligation, Profile, Registry};
use zsolv::util::COIN;
use zsolv::wallet::{make_reserve, transfer, Transfer, Wallet};

const REG: &str = "test-registry";

fn profile(id: &str, name: &str) -> Profile {
    Profile { id: id.into(), name: name.into(), obligations: vec![Obligation { label: "Client balances".into(), zat: 480 * COIN }], ..Default::default() }
}

fn request(fund: &FundKey, authority: &str, round: u64, time: u64) -> RequestBody {
    let pk = fund.public().unwrap();
    let sig = fund.sign(&request_message(REG, &fund.fund_id, authority, round, "Q4 review", time)).unwrap();
    RequestBody { fund_id: fund.fund_id.clone(), authority_key: authority.into(), round, note: "Q4 review".into(), signed: Signed { key: pk, time, signature: sig } }
}

fn fund_read(fund: &FundKey, as_key: &FundKey) -> FundReadBody {
    let (pk, t) = (as_key.public().unwrap(), unix_now());
    let sig = as_key.sign(&fund_read_message(REG, &fund.fund_id, &pk, t)).unwrap();
    FundReadBody { fund_id: fund.fund_id.clone(), signed: Signed { key: pk, time: t, signature: sig } }
}

fn inbox(a: &IssuerKey) -> Signed {
    let (pk, t) = (a.public().unwrap(), unix_now());
    let sig = a.sign(&inbox_message(REG, &pk, t)).unwrap();
    Signed { key: pk, time: t, signature: sig }
}

#[test]
fn certificates_from_the_registry_and_approved_authorities() {
    let log = |_: &str| {};
    let mut l = Ledger::new();
    let mut w = Wallet::generate("northwind");
    transfer(&mut l, Transfer { from: None, from_label: "client".into(), to: &mut w, amount: 500 * COIN, memo: String::new(), reserve: false }).unwrap();
    let x = make_reserve(&mut l, &mut w, 480 * COIN).unwrap();
    let h = l.push_block(vec![], vec![]);

    let fund = FundKey::generate("northwind");
    let other = FundKey::generate("other");
    let mut reg = Registry::new(REG);
    reg.register(sign_profile(REG, &profile("northwind", "Northwind Custody"), &fund, 1).unwrap()).unwrap();
    reg.register(sign_profile(REG, &profile("other", "Other"), &other, 1).unwrap()).unwrap();
    let a = prove(ProveRequest {
        wallet: &w, ledger: &l, registry: REG.into(), fund_id: "northwind".into(), fund_key: &fund, round: 1,
        threshold_zat: 480 * COIN, since: Some(x), at: h, cheat_claim_since: None, log: &log,
    })
    .unwrap();
    let (ok, why, _) = reg.submit(a.clone(), &l).unwrap();
    assert!(ok, "{why}");

    // The registry certifies the accepted round, once.
    let registry = IssuerKey::generate("Z-Solvency registry", "registry");
    let rc = reg.certify_latest("northwind", &registry).unwrap();
    check_certificate(&rc).unwrap();
    assert_eq!((rc.kind.as_str(), rc.round, rc.threshold_zat, rc.checks_passed, rc.checks_total), ("registry", 1, 480 * COIN, 13, 13));
    assert_eq!(reg.certify_latest("northwind", &registry).unwrap(), rc);
    let mut forged = rc.clone();
    forged.threshold_zat = 5000 * COIN;
    assert!(check_certificate(&forged).unwrap_err().to_string().contains("changed"));

    // Certificates never reach the public link page.
    let page = serde_json::to_string(&fund_view(&reg.funds["northwind"], &l, &reg.events)).unwrap();
    assert!(!page.contains("certificate") && !page.contains(&rc.signature));

    // Only the fund reads its certificates.
    let mine = reg.fund_certificates(&fund_read(&fund, &fund)).unwrap();
    assert_eq!(mine["registry_certificates"].as_array().unwrap().len(), 1);
    assert!(reg.fund_certificates(&fund_read(&fund, &other)).unwrap_err().to_string().contains("not signed by the key"));

    // Authorities must be approved before anyone can ask them, or they can read an inbox.
    let auditor = IssuerKey::generate("Acme Audit LLP", "auditor");
    let ak = auditor.public().unwrap();
    assert!(reg.request_certification(request(&fund, &ak, 1, unix_now())).unwrap_err().to_string().contains("isn't approved"));
    assert!(reg.authority_inbox(&inbox(&auditor)).unwrap_err().to_string().contains("isn't approved"));
    assert!(reg.approve_authority(&ak, "Acme Audit LLP", "landlord").is_err());
    reg.approve_authority(&ak, "Acme Audit LLP", "auditor").unwrap();

    // Requests: signed by the fund, fresh, for a published round, not twice.
    assert!(reg.request_certification(request(&other, &ak, 1, unix_now())).unwrap_err().to_string().contains("isn't published"));
    let mut stolen = request(&fund, &ak, 1, unix_now());
    stolen.signed.key = other.public().unwrap();
    assert!(reg.request_certification(stolen).is_err());
    assert!(reg.request_certification(request(&fund, &ak, 1, unix_now() - 3600)).unwrap_err().to_string().contains("too old"));
    assert!(reg.request_certification(request(&fund, &ak, 2, unix_now())).unwrap_err().to_string().contains("isn't published"));
    let rq = reg.request_certification(request(&fund, &ak, 1, unix_now())).unwrap();
    assert!(reg.request_certification(request(&fund, &ak, 1, unix_now())).unwrap_err().to_string().contains("already waiting"));

    // The auditor sees the request with the round's attestation, and checks it itself.
    let ib = reg.authority_inbox(&inbox(&auditor)).unwrap();
    let item = &ib["requests"][0];
    assert_eq!(item["request"]["id"], rq.id.as_str());
    assert_eq!(item["fund_name"], "Northwind Custody");
    let seen: zsolv::attest::Attestation = serde_json::from_value(item["attestation"].clone()).unwrap();
    let v = verify(&seen, &l);
    assert!(v.ok);

    // Another key can't issue or decline for this auditor, even if approved.
    let rival = IssuerKey::generate("Rival Audit", "auditor");
    reg.approve_authority(&rival.public().unwrap(), "Rival Audit", "auditor").unwrap();
    let wrong = issue("authority", &rival, &seen, &v, "Northwind Custody", "").unwrap();
    assert!(reg.issue_certificate(IssueBody { request_id: rq.id.clone(), certificate: wrong }).unwrap_err().to_string().contains("isn't from the authority"));
    let t = unix_now();
    let decl = DeclineBody {
        request_id: rq.id.clone(),
        reason: "no".into(),
        signed: Signed { key: rival.public().unwrap(), time: t, signature: rival.sign(&decline_message(REG, &rq.id, &rival.public().unwrap(), "no", t)).unwrap() },
    };
    assert!(reg.decline_certification(decl).unwrap_err().to_string().contains("another authority"));

    // A certificate edited after signing, or under another name, is refused.
    let mut edited = issue("authority", &auditor, &seen, &v, "Northwind Custody", "Q4 review").unwrap();
    edited.since_height = 0;
    assert!(reg.issue_certificate(IssueBody { request_id: rq.id.clone(), certificate: edited }).is_err());
    let renamed = issue("authority", &IssuerKey { name: "Big Four LLP".into(), ..auditor.clone() }, &seen, &v, "Northwind Custody", "").unwrap();
    assert!(reg.issue_certificate(IssueBody { request_id: rq.id.clone(), certificate: renamed }).is_err());

    // The real one is recorded, and only once.
    let cert = issue("authority", &auditor, &seen, &v, "Northwind Custody", "Q4 review").unwrap();
    let done = reg.issue_certificate(IssueBody { request_id: rq.id.clone(), certificate: cert.clone() }).unwrap();
    assert_eq!(done.status, "issued");
    assert!(reg.issue_certificate(IssueBody { request_id: rq.id.clone(), certificate: cert.clone() }).unwrap_err().to_string().contains("already issued"));
    let mine = reg.fund_certificates(&fund_read(&fund, &fund)).unwrap();
    assert_eq!(mine["requests"][0]["request"]["certificate"]["id"], cert.id.as_str());
    assert_eq!(mine["requests"][0]["authority_name"], "Acme Audit LLP");

    // Declining: only a pending request, with a reason.
    let rq2 = reg.request_certification(request(&fund, &rival.public().unwrap(), 1, unix_now())).unwrap();
    let t = unix_now();
    let reason = "Need the obligations schedule first";
    let decl = DeclineBody {
        request_id: rq2.id.clone(),
        reason: reason.into(),
        signed: Signed { key: rival.public().unwrap(), time: t, signature: rival.sign(&decline_message(REG, &rq2.id, &rival.public().unwrap(), reason, t)).unwrap() },
    };
    assert_eq!(reg.decline_certification(decl).unwrap().status, "declined");

    // Revoked authorities can't issue any more.
    let rq3 = {
        reg.approve_authority(&ak, "Acme Audit LLP", "auditor").unwrap();
        let other_round = request(&fund, &ak, 1, unix_now());
        reg.request_certification(other_round).unwrap()
    };
    reg.revoke_authority(&ak).unwrap();
    let again = issue("authority", &auditor, &seen, &v, "Northwind Custody", "").unwrap();
    assert!(reg.issue_certificate(IssueBody { request_id: rq3.id, certificate: again }).unwrap_err().to_string().contains("no longer approved"));
}
