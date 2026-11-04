export function ListFund() {
  const registry = window.location.origin
  return (
    <>
      <a className="back" href="#/">
        ← Home
      </a>
      <section className="card prose">
        <h1>List your fund</h1>
        <p className="desc">
          The treasurer proves the reserve from the fund console, on their own laptop. The keys stay there; only signed
          proofs come to this registry.
        </p>
        <ol className="howto">
          <li>
            <strong>Get zsolv.</strong> Build it from source (Rust 1.97 or later):
            <pre>cargo build --release</pre>
          </li>
          <li>
            <strong>Start the console.</strong> It opens in your browser, on this machine only:
            <pre>zsolv console</pre>
          </li>
          <li>
            <strong>Set up.</strong> Create the fund's wallet and write down its 24 backup words. They also restore the
            fund key. Use this registry: <code>{registry}</code>
          </li>
          <li>
            <strong>Register.</strong> Name the fund, say what it's for, list its obligations and how long it promises to
            hold the reserve. The page goes live, grey.
          </li>
          <li>
            <strong>Lock the reserve.</strong> One note of exactly the obligations. Pay out from the rest.
          </li>
          <li>
            <strong>Prove and publish</strong> every week. Each round takes seconds and turns the page amber, then green
            once the promise passes.
          </li>
        </ol>
      </section>
    </>
  )
}
