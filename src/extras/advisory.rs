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

/// Community emergency-migration walkthrough. Listed first in the banner because
/// acting matters more than understanding for anyone holding an affected seed.
/// Hosted on X, so it may be unreadable without an account.
pub const MIGRATION_GUIDE_URL: &str =
    "https://x.com/Rob1Ham/status/2083936334511538368";

/// Full-width warning bar rendered above the navbar on every page.
#[component]
pub fn AdvisoryBanner() -> impl IntoView {
    view! {
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
                    "Since March 2021, Coldcards generated seed words from a predictable random number generator instead of the hardware one. Mk2 and Mk3 are worst hit, but Mk4, Q and Mk5 are affected too. Funds are being stolen right now. If you generated a seed on a Coldcard, treat it as compromised and migrate. "
                    <a
                        href=MIGRATION_GUIDE_URL
                        target="_blank"
                        rel="noreferrer"
                        class="font-semibold text-[#ffce6b] underline underline-offset-2 whitespace-nowrap hover:text-white transition-colors"
                    >
                        "How to migrate now \u{2192}"
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
    }
}
