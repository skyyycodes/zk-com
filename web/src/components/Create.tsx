/** How a treasurer gets a share link. The form itself runs in `zsolv console`, on their own laptop. */
export function Create() {
  const registry = window.location.origin
  return (
    <>
      <section className="hero">
        <h1>Create your fund’s link</h1>
        <p className="desc">
          The form runs on your own laptop, in the fund console, because it holds the fund’s keys and makes the proof.
          At the end it gives you one link. Share it with your auditors, clients and counterparties: anyone with the link can check the proof.
          Nobody can find your fund without it.
        </p>
      </section>

      <section className="card prose">
        <h2 className="eyebrow">Five steps, about ten minutes</h2>
        <ol className="howto">
          <li>
            <div>
              <strong>Get zsolv.</strong> Build it from source (Rust 1.97 or later):
              <pre>cargo build --release</pre>
            </div>
          </li>
          <li>
            <div>
              <strong>Start the console.</strong> It opens in your browser, on this machine only:
              <pre>zsolv console</pre>
              <p className="hint">
                When it asks for a registry, use <code>{registry}</code>
              </p>
            </div>
          </li>
          <li>
            <div>
              <strong>Fill in the form.</strong> Create the fund’s wallet and write down its 24 backup words. Then name
              the fund, say what it’s for, list what it owes and how long it promises to hold the reserve.
            </div>
          </li>
          <li>
            <div>
              <strong>Lock the reserve.</strong> One note of exactly the obligations, sent to yourself. Pay out from the
              rest.
            </div>
          </li>
          <li>
            <div>
              <strong>Prove and publish.</strong> It takes seconds. The console shows your link:
              <div className="link-mock" aria-hidden="true">
                <span className="mono">{registry.replace(/^https?:\/\//, '')}/#/v/3f9c…e21a</span>
                <span className="link-mock-btn">Copy link</span>
              </div>
            </div>
          </li>
        </ol>
      </section>

      <section className="card prose">
        <h2 className="eyebrow">About the link</h2>
        <ul className="facts-list">
          <li>
            <strong>Unlisted.</strong> The registry has no list of funds. Your page exists only for people you give the
            link to, and anyone they pass it on to.
          </li>
          <li>
            <strong>The same link every round.</strong> Each new proof shows up on it; nobody needs a new link.
          </li>
          <li>
            <strong>Lost it?</strong> Open the console with the fund’s wallet (or restore it from the backup words), and
            save the fund page again with the same fund id. The console shows the link again.
          </li>
        </ul>
      </section>
    </>
  )
}
