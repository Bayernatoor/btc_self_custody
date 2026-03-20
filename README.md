# WE HODL BTC

Free, opinionated Bitcoin self-custody guides. From beginner mobile wallets to advanced multisig setups.

**Live site:** <https://www.wehodlbtc.com/>

## Tech Stack

- **Language:** Rust
- **Framework:** [Leptos 0.8](https://github.com/leptos-rs/leptos) (SSR + WASM hydration)
- **Server:** Axum
- **Styling:** Tailwind CSS v4
- **Fonts:** Oswald + Questrial

## Project Structure

```
src/
  app.rs          — Router, HTML shell, meta tags
  guides.rs       — Wallet, level, platform definitions (single source of truth)
  lib.rs          — Crate root, WASM hydrate entry point
  main.rs         — SSR server entry point (Axum)
  extras/         — Reusable UI components (navbar, footer, stepper, accordion, buttons, spinner)
  routes/         — Page components (homepage, guide selector, guide pages, FAQ, about, blog)
  helpers/        — Utility modules
  faqs/           — Markdown FAQ/guide content loaded at runtime
style/
  tailwind.css    — Tailwind config, fonts, animations
assets/           — Static assets (JSON-LD, images)
```

## Running Locally

```bash
# Install dependencies
cargo install cargo-leptos

# Development (watches for changes)
cargo leptos watch

# Production build
cargo leptos build --release
```

## Contributing

MIT licensed. Contributions, feedback, and bug reports welcome — please open an [issue on GitHub](https://github.com/Bayernatoor/btc_self_custody).
