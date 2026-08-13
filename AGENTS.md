# AGENTS.md, we_hodl_btc

Rust/Leptos site behind <https://www.wehodlbtc.com/>. Crate `we_hodl_btc` v0.5.0, edition 2021.

House rules: `~/programming/AGENTS.md`. Tech stack and structure: `README.md`. Architecture:
`docs/ARCHITECTURE.md`. Data fields: `docs/DATA_DICTIONARY.md`. This file covers what an agent
needs that those do not: commands, the traps, and where to look.

## Commands

    cargo leptos watch                  # dev server on 127.0.0.1:8000, live reload on 3002
    cargo leptos build --release         # production build

Before pushing, the same three things CI runs, in order:

    cargo fmt --check
    cargo clippy --features ssr --all-targets -- -D warnings
    cargo test --features ssr

`git config core.hooksPath .githooks` wires those up: `pre-commit` runs fmt only (about a
second), `pre-push` runs all three. Green locally then means green in CI. Bypass with
`--no-verify` if you must.

`cargo run --bin backfill_missing_heights` is a one-shot maintenance utility and needs
`--features ssr`.

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

## Deployment

`.github/workflows/deploy.yml` on push to master: builds on the runner, runs `cargo test --lib`,
bumps the service-worker cache version, then rsyncs the binary and `target/site` to the droplet.
`scripts/deploy-remote.sh` then swaps them in atomically, keeping `.prev` copies, restarts the
`wehodlbtc` service, health-checks it, and rolls back to `.prev` on failure.

**The running app lives in `target/` on the droplet.** `cargo clean` there deletes production.
That constraint is remote-only; locally `target/` is disposable and worth cleaning, it reaches
tens of GB.

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
