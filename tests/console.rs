//! The fund console's journey against a running registry on a devnet:
//! set up, receive, register, lock the reserve, prove and publish, and a
//! round saved while the registry is down, published on retry.
use serde_json::Value;
use zsolv::console::{Console, MoveReq, RegisterReq, SetupReq};
use zsolv::ledger::Ledger;
use zsolv::registry::{serve, Obligation, RateLimit};
use zsolv::store::DataDir;
use zsolv::util::COIN;

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn wait_for_job(c: &Console) -> Value {
    for _ in 0..600 {
        let (_, j) = c.handle("GET", "job", "").unwrap();
        if j["running"] == false {
            return j;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    panic!("the proof never finished");
}

#[test]
fn treasurer_journey() {
    let dir = std::env::temp_dir().join(format!("zsolv-console-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let d = DataDir(dir.clone());
    Ledger::new().save(&d.devnet_ledger()).unwrap();
    let port = free_port();
    let url = format!("http://127.0.0.1:{port}");
    {
        let d = d.clone();
        let addr = format!("127.0.0.1:{port}");
        std::thread::spawn(move || serve(d, "zsolv-devnet-registry-1".into(), &addr, "no-web".into(), RateLimit { per_minute: 0, rebuilds_per_hour: 0, trust_proxy: false }));
    }
    for _ in 0..100 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    let c = Console::new(d.clone());
    assert_eq!(c.state().unwrap()["configured"], false);

    // 1. Setup: a new wallet with backup words, on this registry
    let wrong = c.setup(SetupReq { mode: "create".into(), network: "testnet".into(), registry_url: url.clone(), wallet: Some("fund".into()), words: None, birthday: None, server: None });
    assert!(wrong.unwrap_err().to_string().contains("runs on zsolv-devnet"));
    let s = c.setup(SetupReq { mode: "create".into(), network: "devnet".into(), registry_url: url.clone(), wallet: Some("fund".into()), words: None, birthday: None, server: None }).unwrap();
    assert_eq!(s["words"].as_str().unwrap().split_whitespace().count(), 24);

    // money arrives
    c.receive(MoveReq { amount: "530".into(), label: "Dockworkers Local 9".into(), memo: "solidarity".into() }).unwrap();

    // 2. Register: the page goes live, grey
    let obligations = vec![Obligation { label: "Strike pay: 120 workers x 1 ZEC x 4 weeks".into(), zat: 480 * COIN }];
    c.register(RegisterReq { id: "mill".into(), name: "Mill Workers' Strike Fund".into(), description: "Strike pay".into(), obligations, min_held_blocks: 3, max_proof_age_blocks: 8064 }).unwrap();
    let st = c.state().unwrap();
    assert_eq!(st["fund"]["registry"]["status"], "none");
    assert_eq!(st["fund"]["registry"]["key"], st["fund"]["key"], "the registry holds the key derived from this wallet");
    // the share link: an unguessable token, and the only way in
    let share = st["fund"]["page_url"].as_str().unwrap().to_string();
    let link = share.rsplit("/#/v/").next().unwrap().to_string();
    assert!(share.starts_with(&format!("{url}/#/v/")) && link.len() == 32, "{share}");
    let get = |path: &str| ureq::get(&format!("{url}{path}")).call().map(|r| r.status()).unwrap_or_else(|e| match e {
        ureq::Error::Status(code, _) => code,
        e => panic!("{e}"),
    });
    assert_eq!(get(&format!("/api/links/{link}")), 200);
    assert_eq!(get("/api/funds/mill"), 404, "funds aren't reachable by id");
    let state: Value = serde_json::from_str(&ureq::get(&format!("{url}/api/state")).call().unwrap().into_string().unwrap()).unwrap();
    assert!(state.get("funds").is_none() && state.get("events").is_none(), "the registry lists no funds: {state}");

    // 3. Lock exactly T = the obligations
    let r = c.reserve().unwrap();
    assert_eq!(r["locked_zat"], 480 * COIN);
    assert!(c.reserve().unwrap_err().to_string().contains("already locked"));
    let st = c.state().unwrap();
    assert_eq!(st["wallet"]["reserve"]["value_zat"], 480 * COIN);
    assert_eq!(st["wallet"]["operating_zat"], 50 * COIN);

    // payouts come from operating money; one that would touch the reserve is refused
    assert!(c.pay(MoveReq { amount: "60".into(), label: "members".into(), memo: "strike pay".into() }).unwrap_err().to_string().contains("would spend the reserve"));
    c.pay(MoveReq { amount: "30".into(), label: "members".into(), memo: "strike pay wk1".into() }).unwrap();
    c.mine(3).unwrap();

    // 4. Prove and publish
    c.start_prove().unwrap();
    assert!(c.start_prove().is_err(), "one proof at a time");
    let j = wait_for_job(&c);
    assert_eq!(j["error"], Value::Null, "{j}");
    assert_eq!(j["result"]["accepted"], true, "{j}");
    assert_eq!(j["result"]["round"], 1);
    assert!(j["steps"].as_array().unwrap().iter().all(|s| s["state"] == "done"), "{j}");
    assert!(j["steps"][4]["label"].as_str().unwrap().contains("13 of 13"));
    let st = c.state().unwrap();
    assert_eq!(st["fund"]["registry"]["status"], "green", "{}", st["fund"]["registry"]["status_text"]);

    // the registry can't be reached: the round is saved, then published on retry
    c.mine(1).unwrap();
    let cfg_path = d.console();
    let cfg = std::fs::read_to_string(&cfg_path).unwrap();
    std::fs::write(&cfg_path, cfg.replace(&url, &format!("http://127.0.0.1:{}", free_port()))).unwrap();
    c.start_prove().unwrap();
    let j = wait_for_job(&c);
    assert_eq!(j["result"]["saved"], true, "{j}");
    assert!(j["steps"][5]["detail"].as_str().unwrap().starts_with("Saved round 2"), "{j}");
    std::fs::write(&cfg_path, &cfg).unwrap();
    assert_eq!(c.state().unwrap()["pending"].as_array().unwrap().len(), 1);
    let r = c.retry().unwrap();
    assert_eq!(r["results"][0]["accepted"], true, "{r}");
    let st = c.state().unwrap();
    assert!(st["pending"].as_array().unwrap().is_empty());
    assert_eq!(st["fund"]["registry"]["rounds"], 2);

    // a second console opening the same wallet: nothing is listed, so it registers
    // the same id again; the derived key matches and the registry returns the same link
    std::fs::remove_file(&cfg_path).unwrap();
    let s = c.setup(SetupReq { mode: "existing".into(), network: "devnet".into(), registry_url: url, wallet: Some("fund".into()), words: None, birthday: None, server: None }).unwrap();
    assert_eq!(s["fund_id"], Value::Null);
    let obligations = vec![Obligation { label: "Strike pay: 120 workers x 1 ZEC x 4 weeks".into(), zat: 480 * COIN }];
    let r = c.register(RegisterReq { id: "mill".into(), name: "Mill Workers' Strike Fund".into(), description: "Strike pay".into(), obligations, min_held_blocks: 3, max_proof_age_blocks: 8064 }).unwrap();
    assert_eq!(r["page_url"], share);
    assert_eq!(c.state().unwrap()["fund"]["registry"]["rounds"], 2);
    let _ = std::fs::remove_dir_all(dir);
}
