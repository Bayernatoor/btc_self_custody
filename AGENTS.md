# AGENTS.md, we_hodl_btc

Rust/Leptos site behind <https://www.wehodlbtc.com/>. Crate `we_hodl_btc` v0.5.0, edition 2021.

House rules: `~/programming/AGENTS.md`. Tech stack and structure: `README.md`. Architecture:
`docs/ARCHITECTURE.md`. Data fields: `docs/DATA_DICTIONARY.md`. This file covers what an agent
needs that those do not: commands, the traps, and where to look.

## Commands

    cargo leptos watch                  # dev server on 127.0.0.1:8000, live reload on 3002
    ./target/debug/we_hodl_btc          # the built binary, which listens on 3000, not 8000
    cargo leptos build --release         # production build

Before pushing, the same three things CI runs, in order:

    cargo fmt --check
    cargo clippy --features ssr --all-targets -- -D warnings
    cargo test --features ssr

`git config core.hooksPath .githooks` wires those up: `pre-commit` runs fmt only (about a
second), `pre-push` runs all three. Green locally then means green in CI. Bypass with
`--no-verify` if you must.

**Those three prove the code compiles and does what its tests assert. They prove nothing about
what an endpoint returns.** Run the app and call it before pushing:

    cargo leptos watch                  # one terminal
    bash scripts/probe-endpoints.sh     # another; read-only, all 37 endpoints

On 2026-09-08 that found five bugs the three gates had passed clean, across twelve reviewed
branches and 268 tests:

- `/api/stats/signaling?bit=99` returned **nothing at all**. `1i64 << bit` from an unvalidated
  query parameter panics in a debug build and takes the connection with it; in release it masks
  to `bit % 64` and answers as a different bit, which is worse.
- Ten server functions answered a reversed range with `500`, at twelve call sites. A wrong status
  is still a valid response, so no test noticed. Only half of this is fixable here: `server_fn`
  0.8.11 hardcodes `INTERNAL_SERVER_ERROR` for every `ServerFnError`, so these still return 500.
  They now log at `warn` instead of `error`, which was the part doing damage, since a caller's
  bad range was burying real faults. A true 4xx needs a custom `FromServerFnError` type.
- `/blocks/{height}` for a missing height returned `200` carrying `{"error": ...}`.
- A box-drag on any chart zoomed the value axis as well as time, clipping the top bands off a
  stacked chart while the tooltip kept reporting them. The code, the option and the data were all
  correct; only the rendered view was wrong, so nothing static could have caught it.
- Both external price fetches were failing on the dev box, silently degrading the price overlay,
  Almanac prices and the Logbook's price columns to zero rather than surfacing an error. Cause was
  IPv6: hyper has no happy-eyeballs fallback, so a blackholed AAAA route (ProtonVPN's leak
  protection here) stalls a request that curl completes fine over IPv4. Observed on localhost and
  never tested against prod, which does not run ProtonVPN, so production may be unaffected.

Assert status codes in the integration tests when you add an endpoint, and keep the probe script
current: a check that only ever runs by hand is one that stops running.

The probe lives in `scripts/` rather than `notes/` on purpose. `notes/`, `tasks/` and `docs/` are
all gitignored, so anything referenced from this file has to sit somewhere tracked or the
instruction is dead on a fresh clone.

`cargo run --bin backfill_missing_heights` is a one-shot maintenance utility and needs
`--features ssr`.

### The debug binary can fail to link, and the fix is `debug = 0`

**Observed 2026-09-14.** `cargo build --features ssr --bin we_hodl_btc` failed at the link step
with

    rust-lld: error: ...(.debug_str): section too large

and, at `debug = "line-tables-only"`, with hundreds of

    relocation R_X86_64_32 out of range: 4313024424 is not in [0, 4294967295]

which is the same cause seen twice: the debug info across the linked rlibs exceeds the 4 GB
addressable by 32-bit relocations. At full `debug = 2` on thunder it did not reach the linker at
all, getting SIGKILLed by the OOM killer instead (31 GB RAM, ~17 GB free).

`target/debug/incremental` had grown to **46 GB**, which is worth clearing on its own, but doing so
did not fix the link. What works:

    CARGO_PROFILE_DEV_DEBUG=0 cargo build --features ssr --bin we_hodl_btc

That is the probe as well as the fix, and it links in about 90 seconds. Nothing is committed to
change the profile, because a debugger-less dev build is the wrong default to impose; reach for the
env var when you need to run the binary.

**Untested:** whether `cargo leptos watch` hits the same wall. It builds the same bin, so expect
it to, and use the env var there too if it does. The lib, all tests and both `cargo check` targets
are unaffected, so the three CI gates never see this.

## The toolchain pin is deliberate

`rust-toolchain.toml` pins **1.97.1 exactly**, and nothing in `.github/workflows` names a
toolchain, so that file is the only place it is defined. It is an exact version rather than a
channel on purpose: with `stable`, a machine that has not run `rustup update` gets an older
clippy than the CI runner, so lints fire in CI that cannot be reproduced locally.

Bumping it is its own change: edit the version, run fmt and clippy, fix what the newer lints
find, commit that alone.

## Feature flags: the main trap

Two builds from one crate:

- `ssr` (default), server. Pulls in axum, tokio, rusqlite, r2d2, reqwest, zeromq, sha2,
  tower-http, tracing-subscriber, thiserror, futures.
- `hydrate`, the WASM bundle. None of the above exist.

**Anything touching `stats::{db,rpc,config,ingest,zmq_subscriber}` is ssr-only.** Referencing it
from code that also compiles to WASM breaks the hydrate build, and the error surfaces as a
confusing wasm-bindgen or missing-symbol failure rather than a clear one. Check both builds
before assuming a change is fine. `cargo clippy --features ssr` alone will not catch it.

## Leptos, WASM and JS interop

Each of these cost a real debugging session. They fail silently or with an error pointing
somewhere else, which is why they are written down.

**No inline content inside `<script>` tags in the `view!` macro.** Especially JSON or raw
strings. It breaks hydration and the whole app goes non-interactive with no error. Use external
`.js` files.

**`class:` directives cannot take Tailwind classes containing `/` or `[]`.** So no
`class:bg-white/10` or `class:bg-[#f7931a]`. Use a computed attribute instead:
`class=move || format!(...)`.

**A `forwards`-filled opacity animation keeps its stacking context after it finishes, so a
`fixed` overlay inside one cannot escape it.** `ObservatoryPage` wraps every observatory page in
`<section class="... opacity-0 animate-fadeinone">`. That section goes on creating a stacking
context long after the fade ends, which confines any `position: fixed` descendant to it, and the
sticky navbar (`z-30`, root stacking context) then paints over the descendant no matter how high
its z-index goes. The chart drawer sat at `z-index: 10003` and still lost. Probe, three cases side
by side in one static HTML file rendered headless: a `fixed` child of a plain ancestor overlays the
navbar, a child of the finished animation does not. Fix is `leptos::portal::Portal` to `<body>`
(precedent: the `stepper_v2` lightbox). Note `Portal` takes children as `Fn`, so build any `Vec`
inside the closure rather than capturing one, or you get `E0525 ... only implements FnOnce`.

**Never offset a fixed overlay by a hardcoded navbar height.** `top-[48px]` on the chart drawer was
already short of the ~53px navbar (~65px at `2xl`, from `py-4` plus `text-2xl`), and the advisory
banner sits *above* the navbar, so it pushed the navbar down and buried the drawer's first ~120px.
Anything anchored below the navbar has to be full-height-with-a-portal, or measured. The mobile
menu in `navbar.rs` carries the same lesson: it uses `absolute top-full` against the sticky navbar
rather than `fixed top-12`.

**`#![recursion_limit = "512"]` must be in both `lib.rs` and `main.rs`.** The bin crate compiles
separately and does not inherit the lib's limit. Testing only the lib locally means this first
appears in a CI or Docker build.

**A `<Show>` that is not in the SSR HTML does not get event handlers attached** when inserted
after hydration. The heartbeat hint overlay must start `signal(true)`; starting it `false` and
toggling after init produced an undismissable overlay because the click handler never attached.

**For JS touching Leptos-rendered `inner_html`, poll rather than observe.** `MutationObserver`
does not reliably catch Leptos hydration timing; `setInterval` does. Applies to the lightbox and
collapsible sections.

**Compute in Rust, not JS.** JS is for rendering and animation only. `heartbeat.js` reported
average block time as always 10 minutes because it re-derived a value Rust already had correct
from `LiveStats`. Pass computed values into JS; do not recompute them there.

**`prop:value` on a `<select>` does not survive SSR.** It is applied after hydration, so a page
rendered with a selection paints the control empty until WASM loads. Pair it with `selected` on
the options, which is an attribute and does render.

**A native `<select>` popup ignores your CSS.** The browser draws the option list, so a dark
`<select>` opened to a white list with white text, invisible. `[color-scheme:dark]` on the select
is what actually fixes it; explicit `bg-`/`text-` on each `<option>` and `<optgroup>` is the
fallback for platforms that honour one and not the other.

**Anything absolutely positioned inside arbitrary copy has to reset what it inherits.** A tooltip
placed next to a label picked up `white-space: nowrap` from an ancestor and ignored its own
width, running off the screen. Reset `whitespace`, `text-transform`, `font-weight`, `letter-spacing`
and text alignment on a floating element, not just its colours. Related: open such a thing
**downward**. Upward overflow at the top of the page goes behind the navbar and then off the
document, where it cannot be scrolled to; downward overflow is always reachable.

**`node --check` proves nothing about a JS module.** It validates syntax, and an undefined
reference is valid syntax. A broken heartbeat page shipped on 2026-08-07 because a scripted
`replace()` matched nothing (its anchor comment had already been rewritten), leaving `PARAMS`
referencing five constants that were never declared. `node --check` passed, `cargo check` passed,
and the page died at module-eval time. ESM failure is fatal to the whole graph, so
`heartbeat-audio.js` throwing meant `heartbeat.js` never defined its `window.*` functions and the
page hung on "Mining blocks..." with an unrelated-looking error.

After touching a module's top level, actually evaluate it:

    # /tmp/claude/probe.mjs
    globalThis.localStorage = { getItem: () => null, setItem: () => {} };
    globalThis.window = {};
    const m = await import(process.argv[2]);   # file:// URL

Two habits that follow: assert on every scripted replacement so a stale anchor fails loudly, and
prefer an editor-style edit over `sed`/`python` rewrites when the anchor may already have changed.

## ECharts

Three things about the library that are not bugs in our code and cost a session each to work out.

**A log axis labels every decade only when its bounds are exact powers of the base.** Otherwise it
labels the minimum, 1, and the maximum, and nothing else, however wide the span. Measured
2026-09-15 in Chrome 153 against ECharts 5.6.0, reading `axis.getViewLabels()` off the model:

    min 1,        max 1e8     -> 9 labels, one per decade
    min 0.000285, max 2.29e7  -> 3 labels: min, 1, max      (same span, fitted bounds)

`splitNumber` makes **no difference to either case**, at any height from 200px to 3200px. An
earlier version of this section said it was "a hint it can only honour by choosing how many
decades to step", which sounds right and is not what governs the label count. ECharts 5 has no
explicit tick list for a value or log axis, so bounds are the only lever, and **minor ticks**
(`minorTick` plus `minorSplitLine`, at 2, 3, 4 within each decade) are the only way to add
structure between labels.

That makes the sparse axis a direct cost of fitting bounds to data, which `set_axis_scale` does
deliberately: over 1Y difficulty spans 140T to 160T, under one decade, and rounding out to
enclosing decades pins every point to the axis floor. A span-dependent rule would get both.

Two consequences worth knowing before touching axis code. **The bound you set is not the bound
ECharts reports**, because a log axis holds its extent as an exponent: 22,900,000 comes back as
`10^log10(22900000)` = 22900000.00000002 and prints in full, and 140e12 comes back as
139,999,999,999,999. An `axisLabel.formatter` is therefore load-bearing rather than cosmetic on
every log axis. And the site loads **`echarts@5`, a floating major**, so there is no pinned
version; `echarts.version` reported 5.6.0 on 2026-09-15 while a version string inside the bundle
reads 5.6.1. Trust the runtime value.

Probe: `notes/chart-quality-2026-09-15/runs/axis-label-probe.md` and the pages beside it.

**A JS function cannot be serialised from Rust, so use a sentinel.** Rust writes a marker string
where the function belongs, and `stats.js` swaps in the real one. Used for SI axis labels, date
formats and the calendar-tick `axisLabel.interval` predicate. Any new one needs its constant
declared on both sides and a test asserting the option carries both halves.

**A time axis takes a per-level format object**, keyed `year`/`month`/`day`/`hour`/..., which is
plain JSON and needs no sentinel. Without it every tick level is formatted by one string, so a
one-day range shows no clock time and a one-week range shows bare day numbers.

## Content and data rules

The site is meant to read as authoritative reference, not as a blog.

**Verify every number.** Never take a block height, txid, fee, date or record from training data.
Query the local database first (`SELECT height, size FROM blocks ORDER BY size DESC LIMIT 1`), or
verify against multiple sources. A previous "largest block" claim was wrong: training data said
774,628, the database says 836,964 at 3.99 MB. Inaccurate data destroys the site's credibility.
If uncertain, flag it rather than guessing.

**No opinions on `/observatory` pages** unless mathematically provable or undisputed. Not
"Bitcoin's most significant upgrade since its creation" but "one of Bitcoin's largest protocol
upgrades". Stick to verifiable facts: heights, dates, tx counts, BIP numbers.

**Tone:** professional and objective but not boring. Hedged where hedging is honest (can,
suggests, tends to, indicates). No absolutes unless factual, nothing exaggerated or superfluous.
Match the existing copy rather than introducing a new voice.

## Layout

    src/app.rs              router, HTML shell, meta
    src/guides.rs           wallet/level/platform definitions, single source of truth
    src/guides_v2.rs        the v2 guide model
    src/routes/             pages; routes/observatory/ is the data-visualisation section
    src/stats/              the whole data subsystem, ssr-only
    src/stats/charts/       chart builders per topic
    src/extras/             shared UI (navbar, footer, stepper, accordion, spinner, schema)
    src/helpers/markdown.rs markdown rendering for faqs/
    src/faqs/               markdown content loaded at runtime
    style/tailwind.css      Tailwind v4 config, fonts, animations
    tasks/                  todo.md and lessons.md, live state, scanned by master-list
    docs/                   architecture, data dictionary, heartbeat designs, ops runbooks
    notes/                  design system, guide specs, mockups

## Data subsystem

Reads from your own bitcoind over RPC and ZMQ into SQLite. Configuration is entirely by
environment:

    BITCOIN_STATS_DB_PATH          BITCOIN_STATS_RPC_URL
    BITCOIN_STATS_RPC_USER         BITCOIN_STATS_RPC_PASSWORD
    BITCOIN_STATS_RPC_CONCURRENCY  BITCOIN_STATS_INITIAL_INGEST
    BITCOIN_STATS_ZMQ_BLOCK        BITCOIN_STATS_ZMQ_TX
    BITCOIN_STATS_ZMQ_SEQUENCE

`*.db`, `*.db-shm`, `*.db-wal` are gitignored. `bitcoin_stats.db` is ~500 MB locally and is not
reproducible quickly, so do not delete it casually.

**`WHERE height % 2016 = 0` cannot use an index, so generate the heights instead.** A recursive CTE
producing multiples of 2,016 and joining `blocks` by primary key measured **2.1ms warm against
19.3ms** for the modulo form on 967,000 rows (2026-09-16); `query_retargets_for_window` is the
worked example. The same shape applies to any "every Nth height" question.

**A daily mean cannot recover a per-block event, and difficulty is the worked example.** A retarget
lands at an arbitrary moment, so the day holds blocks from two epochs and its mean is neither of
them. Worse, block 899,136 is stamped 2025-05-31 00:01:30, so that day carries only the new
difficulty and is indistinguishable from a blend day. Read the event's own row; do not infer it
from `daily_blocks`.

## Deployment

`.github/workflows/deploy.yml` on push to master: builds on the runner, runs `cargo test --lib`,
bumps the service-worker cache version, then rsyncs the binary and `target/site` to the droplet.
`scripts/deploy-remote.sh` then swaps them in atomically, keeping `.prev` copies, restarts the
`wehodlbtc` service, health-checks it, and rolls back to `.prev` on failure.

**The running app lives in `target/` on the droplet.** `cargo clean` there deletes production.
That constraint is remote-only; locally `target/` is disposable and worth cleaning, it reaches
tens of GB.

**The droplet is 2 vCPU / 4GB RAM / 80GB NVMe** (resized 2026-07-21 from 2GB/60GB, which swapped
continuously). Treat 4GB as the memory budget rather than headroom: the SSE heartbeat history
payload is tens of MB per build, and any RPC response parsed into a `serde_json::Value` instead
of a typed struct spikes proportionally to the response (`rpc::call_typed` exists for exactly
that reason). This lives here rather than only in `docs/ARCHITECTURE.md` because `docs/` is
gitignored, so nothing in it reaches a fresh clone or another machine.

## Conventions

- **No infrastructure scripts in this repo.** iptables rules, cron jobs, systemd units, WireGuard
  configs and node setup scripts belong on the server or in a separate infra repo. Reference
  material for the Start9 and droplet setup goes in `docs/` as documentation, or stays local.
- `master` is kept linear. Rebase feature branches; never merge master into one.
- Never push without being asked. Stage work and hand over the commit command; GPG signing
  fails in sandboxed runs.
- Comments explain why the current code is as it is, never how it got there.
- Prefer writing under 50 lines to adding a dependency. Lock exact versions.
- Never drop a table. `ALTER TABLE` plus a backfill.
- Compute in Rust, not JS. JS is for rendering only.
- `tasks/lessons.md` gets updated right after a correction, not at session end.

## Known gaps

- `Cargo.toml` sets `end2end-dir = "end2end"` but the directory on disk is `e2e/`, so
  `cargo leptos end-to-end` looks in a path that does not exist. Playwright config lives in
  `e2e/playwright.config.ts`; `.github/workflows/e2e.yml` runs it directly.
- `pathfinder/` and this repo were the only `active/bitcoin` projects without an `AGENTS.md`.
  This file closes half of that.
