//! Temporary site-wide security advisory banner.
//!
//! Added 2026-07-31, the day after Coinkite disclosed a seed-generation entropy
//! flaw: the RNG hashed the device-generated seed together with every dice roll,
//! leaving affected seeds with roughly 72 bits of entropy instead of 128.
//!
//! Affected: Coldcard Mk3 on any firmware from 4.0.1 (March 2021) onward, Mk4 and
//! Mk5 before 5.6.0, and Q before 1.5.0Q. TAPSIGNER, OPENDIME and SATSCARD are not
//! affected. Updating firmware does NOT repair a seed that was already generated.
//!
//! To retire this: delete this file, its `pub mod advisory;` line in extras/mod.rs,
//! and the `<AdvisoryBanner/>` in app.rs.

use leptos::prelude::*;

/// Coinkite's advisory. Kept as a constant so the guides can link the same source.
pub const COLDCARD_ADVISORY_URL: &str =
    "https://blog.coinkite.com/coldcard-mk3-seed-generation-warning/";

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
                    "A flaw in the way Coldcards generated seed words left them weaker than they should be. If your seed was created on a Mk3 running any firmware from 4.0.1 (March 2021) onward, treat this as urgent: your funds could be at risk. Mk4, Mk5 and Q owners on older firmware are also vulnerable. "
                    <a
                        href=COLDCARD_ADVISORY_URL
                        target="_blank"
                        rel="noreferrer"
                        class="font-semibold text-[#ffce6b] underline underline-offset-2 whitespace-nowrap hover:text-white transition-colors"
                    >
                        "Read Coinkite's advisory \u{2192}"
                    </a>
                </p>
            </div>
        </aside>
    }
}
