//! Temporary site-wide security advisory banner.
//!
//! Added 2026-07-31 after the COLDCARD seed-generation failure.
//!
//! Root cause (per Block Engineering, who found it): a macro-check bug bound libngu
//! to MicroPython's FALLBACK RNG rather than the hardware RNG, so seeds derived from
//! observable device state (chip UID, SysTick, RTC). On current models the
//! secure-element reseed contributes only 32 bits, into one state word.
//!
//! Severity: Block puts Mk2/Mk3 v4.0.0-v4.1.9 near 2^16 on a normal cold boot and
//! bounds current Mk4/Q/Mk5 near 2^32, i.e. brute-forceable. Coinkite's own advisory
//! quoted ~72 bits and offered dice rolls / a passphrase as mitigations; Block's
//! analysis does not support that framing, so do NOT repeat it. Present since
//! v4.0.0 (17 March 2021). Active exploitation confirmed on disclosure day.
//! TAPSIGNER, OPENDIME and SATSCARD are not affected. Updating firmware does NOT
//! repair a seed that was already generated.
//!
//! To retire this: delete this file, its `pub mod advisory;` line in extras/mod.rs,
//! and the `<AdvisoryBanner/>` in app.rs.
//!
//! Scoped to the guide pages on 2026-10-02. It was site-wide for two months,
//! which was right while the disclosure was fresh. The hazard has not expired,
//! so the banner stays where someone is choosing or setting up a wallet.

use leptos::prelude::*;

/// Coinkite's own advisory. Kept because the guides link it, but note it understates
/// the severity and offers dice/passphrase mitigations that Block's analysis does not
/// support. Prefer BLOCK_REPORT_URL when pointing readers at one source.
pub const COLDCARD_ADVISORY_URL: &str =
    "https://blog.coinkite.com/coldcard-mk3-seed-generation-warning/";

/// Block Engineering's technical report. They found the bug; this is the accurate
/// account of the root cause and the real search-space numbers.
pub const BLOCK_REPORT_URL: &str =
    "https://engineering.block.xyz/blog/predictable-rng-fallback-and-32-bit-reseed-in-coldcard-firmware";

/// Rob Hamilton's triage thread. Basis for the urgency levels on /coldcard-migration, and
/// where we send readers for named product opinions. Hosted on X, so it may be
/// unreadable without an account; that is why /coldcard-migration exists rather than linking
/// this directly from the banner.
pub const ROB_THREAD_URL: &str =
    "https://x.com/Rob1Ham/status/2083936334511538368";

/// Full-width warning bar above the navbar, on the guide pages only.
///
/// Site-wide until 2026-10-02. It is a warning about one vendor's hardware,
/// so it belongs where someone is choosing or setting up a wallet, and the
/// guides are the only pages that ask that question. Everywhere else it was
/// the first thing a new reader saw: on a phone it took about 110px of an
/// 844px viewport, on every page, with no way to dismiss it.
///
/// Scoped rather than deleted because the hazard has not expired. Updating
/// firmware does not repair a seed that was already generated, so anyone
/// arriving at a Coldcard guide still needs to see this first.
///
/// `/coldcard-migration` is included because it is the advisory's own page
/// and the banner's links point into it.
#[component]
pub fn AdvisoryBanner() -> impl IntoView {
    let location = leptos_router::hooks::use_location();
    let show = Signal::derive(move || {
        let path = location.pathname.get();
        path.starts_with("/guides") || path == "/coldcard-migration"
    });
    // `.then(...)`, not `<Show>`. A `<Show>` here rendered nothing at all
    // under SSR even with its condition true, so the banner was absent
    // from the served HTML on the very pages it exists for. The plain
    // conditional is what the rest of this codebase uses and it renders
    // server-side.
    view! {
        {move || show.get().then(|| view! {
        <aside
            aria-label="Security advisory"
            class="w-full bg-[#ffce6b]/10 border-b border-[#ffce6b]/25 px-4 py-2.5 sm:px-6"
        >
            <div class="max-w-5xl mx-auto flex items-start gap-2.5 lg:max-w-6xl">
                <svg
                    class="w-4 h-4 mt-0.5 shrink-0 text-[#ffce6b]"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                    aria-hidden="true"
                >
                    <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="2"
                        d="M12 9v4m0 4h.01M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z"
                    />
                </svg>
                <p class="text-[0.8rem] leading-relaxed text-[#f0d9a8] sm:text-[0.85rem]">
                    <span class="font-semibold text-[#ffce6b]">
                        "Security advisory: Coldcard seed generation. "
                    </span>
                    // Cut to one line on 2026-09-11, six weeks after the
                    // disclosure. The detail moved to /coldcard-migration
                    // rather than being deleted: a banner that stays paragraph-
                    // length past the acute phase gets scrolled past, and it
                    // costs every page above the fold. What survives is the
                    // instruction, since that is the part a reader has to act
                    // on; the model list and the mechanism are one click away.
                    "If you generated a seed on a Coldcard, treat it as compromised and migrate. "
                    <a
                        href="/coldcard-migration"
                        class="font-semibold text-[#ffce6b] underline underline-offset-2 whitespace-nowrap hover:text-white transition-colors"
                    >
                        "What to do \u{2192}"
                    </a>
                    <span class="text-[#ffce6b]/40 px-1.5" aria-hidden="true">"|"</span>
                    <a
                        href=BLOCK_REPORT_URL
                        target="_blank"
                        rel="noreferrer"
                        class="font-semibold text-[#ffce6b] underline underline-offset-2 whitespace-nowrap hover:text-white transition-colors"
                    >
                        "Technical report \u{2192}"
                    </a>
                </p>
            </div>
        </aside>
        })}
    }
}
