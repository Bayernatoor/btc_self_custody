//! Emergency advisory page for the COLDCARD seed-generation failure (2026-07-30).
//!
//! Editorial position: anyone whose seed was generated on a
//! COLDCARD running March 2021 firmware or later should migrate, regardless of dice
//! or passphrase. Dice and passphrases change how URGENT that is, not whether it is
//! needed. That framing is deliberate: it does not depend on whose entropy analysis
//! turns out to be right, and it fails safe.
//!
//! Retire alongside src/extras/advisory.rs when the advisory is no longer current.

use leptos::prelude::*;
use leptos_meta::*;

use crate::extras::advisory::{BLOCK_REPORT_URL, COLDCARD_ADVISORY_URL, ROB_THREAD_URL};

/// Bump this whenever the page changes. Readers of a developing advisory need to
/// know how fresh the guidance is.
const LAST_UPDATED: &str = "3 August 2026";

/// MARA's direct-to-miner submission service, opened to the public during this
/// incident so anyone can bypass the public mempool without an account.
const SLIPSTREAM_URL: &str = "https://slipstream.mara.com";

/// Flaxman's multi-vendor multisig guide: the best mental model for a rebuild, but it
/// predates this failure and recommends the COLDCARD. Never link it without that caveat.
const FLAXMAN_GUIDE_URL: &str = "https://btcguide.github.io/";

/// Bits of entropy per unit, so the table is computed rather than hardcoded.
const CHARSETS: &[(&str, f64)] = &[
    ("Random BIP39 seed words", 11.000),
    ("Random words from a long list", 12.925),
    ("Lowercase letters only", 4.700),
    ("Lowercase letters + digits", 5.170),
    ("Upper + lower + digits", 5.954),
    ("Full printable ASCII", 6.555),
];
const LENGTHS: &[usize] = &[8, 12, 16, 20, 25, 30];
const DICE_BITS: f64 = 2.585;

fn bits_class(bits: f64) -> &'static str {
    if bits >= 128.0 {
        "text-[#42d69a]"
    } else if bits >= 100.0 {
        "text-[#ffce6b]"
    } else {
        "text-white/35"
    }
}

/// The situations at each level of urgency, numbered on the page so a reader can point
/// at "case 2" rather than describing a paragraph.
const CASES_NOW: &[&str] = &[
    "A single-signature wallet with no dice and no passphrase.",
    "A multisig made only of affected COLDCARDs, where none of them had dice or a passphrase. Every key in the quorum can be recreated, so needing several of them protects you from nothing.",
    "Any of the above where you have reused a receive address. See the note below, this is worse than it sounds.",
];
const CASES_DAYS: &[&str] = &[
    "Single-sig where you rolled fewer than 50 dice, or where your passphrase is short, guessable, or something you invented rather than generated.",
    "Multisig where the affected COLDCARDs alone can move the funds, for example two COLDCARDs in a 2-of-3.",
];
const CASES_SOON: &[&str] = &[
    "You rolled at least 50 dice (100 is a comfortable margin) and know it for a fact, or your passphrase genuinely carries 128 bits.",
    "A multisig where the affected devices are a minority and cannot move funds on their own. Your bitcoin is safe, but you are running with less redundancy than you think.",
];

/// Numbered list of cases. `chip` carries that level's accent colours.
fn case_list(items: &'static [&'static str], chip: &'static str) -> impl IntoView {
    view! {
        <ol class="space-y-2.5">
            {items
                .iter()
                .enumerate()
                .map(|(i, text)| {
                    view! {
                        <li class="flex gap-2.5 text-sm text-white/75 leading-relaxed">
                            <span class=format!(
                                "shrink-0 mt-0.5 w-5 h-5 rounded-md grid place-items-center text-[0.7rem] font-semibold tabular-nums {}",
                                chip,
                            )>
                                {format!("{}", i + 1)}
                            </span>
                            <span>{*text}</span>
                        </li>
                    }
                })
                .collect::<Vec<_>>()}
        </ol>
    }
}

#[component]
fn Section(
    id: &'static str,
    title: &'static str,
    children: Children,
) -> impl IntoView {
    // scroll-mt clears the sticky navbar (plus the advisory banner above it) so an
    // anchored jump does not land with the heading hidden underneath.
    view! {
        <section id=id class="mb-10 scroll-mt-28">
            <h2 class="group text-xl font-title text-white mb-3 lg:text-2xl">
                <a href=format!("#{}", id) class="hover:text-[#f7931a] transition-colors">
                    {title}
                    <span class="ml-1.5 text-base text-white/20 opacity-0 transition-opacity group-hover:opacity-100">
                        "#"
                    </span>
                </a>
            </h2>
            {children()}
        </section>
    }
}

#[component]
pub fn ColdcardAdvisoryPage() -> impl IntoView {
    view! {
        <Title text="COLDCARD Seed Vulnerability: What To Do | We Hodl BTC"/>
        <Meta
            name="description"
            content="COLDCARD generated wallet seeds from a predictable random number generator from March 2021 onward. Work out how urgently you need to move your bitcoin, and what to move it to."
        />
        <Link rel="canonical" href="https://www.wehodlbtc.com/coldcard-migration"/>

        <div class="max-w-3xl mx-auto mt-10 mb-24 px-6 opacity-0 animate-fadeinone lg:px-8 md:mt-14">

            // ---------- header ----------
            <header class="mb-10">
                <div class="font-title text-xs uppercase tracking-widest text-[#ffce6b] mb-2">
                    "Security advisory"
                    <span class="text-white/35">{format!("  \u{00b7}  Updated {}", LAST_UPDATED)}</span>
                </div>
                <h1 class="text-3xl font-title text-white leading-tight mb-4 lg:text-4xl">
                    "Your COLDCARD seed was not random"
                </h1>
                <p class="text-base text-white/75 leading-relaxed">
                    "Your seed words are supposed to come from pure chance, so that nobody could ever
                     reproduce them. A software bug meant COLDCARDs were not doing that. From March 2021 onward
                     they built seeds out of information about the device itself, such as its serial number and
                     internal clock, which is predictable. That left a small enough pool of possible seeds that
                     thieves can work through it and empty the wallets they find, which is happening now."
                </p>
            </header>

            // ---------- the one-line gate ----------
            <div class="bg-white/5 border border-white/10 rounded-xl p-5 mb-10">
                <p class="text-sm text-white/70 leading-relaxed">
                    <span class="text-white font-semibold">"Not affected? "</span>
                    "If you have never generated a seed on a COLDCARD Mk2, Mk3, Mk4, Mk5 or Q, this does not
                     apply to you and you can stop reading. Seeds you generated somewhere else and merely
                     imported into a COLDCARD are also fine. TAPSIGNER, OPENDIME and SATSCARD are unaffected."
                </p>
            </div>

            // ---------- my position ----------
            <Section id="position" title="My position">
                <div class="border-l-2 border-[#ffce6b] pl-4">
                    <p class="text-base text-white/80 leading-relaxed mb-3">
                        "If your seed was generated on a COLDCARD on firmware from March 2021 or later, move
                         your bitcoin to a new wallet with a new seed. That applies whether or not you rolled
                         dice and whether or not you use a passphrase."
                    </p>
                    <p class="text-sm text-white/60 leading-relaxed">
                        "Dice and passphrases change how fast you need to act, not whether you need to act.
                         Enough dice entropy probably means your seed was fine. A strong passphrase means the
                         passphrase is now the only thing protecting you, on a seed an attacker may be able to
                         reconstruct. Neither is a reason to leave coins where they are indefinitely."
                    </p>
                </div>
            </Section>

            // ---------- urgency ----------
            <Section id="urgency" title="How fast do you need to move?">
                <p class="text-sm text-white/60 leading-relaxed mb-5">
                    "Work down the list and stop at the first one that describes you. Adapted from "
                    <a
                        class="text-[#f7931a] hover:text-[#f4a949] underline underline-offset-2 transition-colors"
                        href=ROB_THREAD_URL
                        target="_blank"
                        rel="noreferrer"
                    >
                        "Rob Hamilton's triage thread"
                    </a>
                    "."
                </p>

                <div class="flex flex-col gap-4">
                    // right now
                    <div class="bg-[#ffce6b]/[0.07] border border-[#ffce6b]/30 rounded-xl p-5">
                        <div class="font-title text-xs uppercase tracking-widest text-[#ffce6b] mb-2">
                            "Right now, today"
                        </div>
                        {case_list(CASES_NOW, "bg-[#ffce6b]/15 text-[#ffce6b]")}
                        <p class="text-sm text-[#ffce6b] mt-3">
                            "Find someone to help you if you need it, but do not wait."
                        </p>
                    </div>

                    // urgently
                    <div class="bg-white/5 border border-white/10 rounded-xl p-5">
                        <div class="font-title text-xs uppercase tracking-widest text-[#f7931a] mb-2">
                            "Urgently, within days"
                        </div>
                        {case_list(CASES_DAYS, "bg-[#f7931a]/15 text-[#f7931a]")}
                        <p class="text-sm text-white/60 mt-3">
                            "Assume the seed is known and that only your passphrase is holding. Use the table
                             below to work out what your passphrase is actually worth."
                        </p>
                    </div>

                    // soon
                    <div class="bg-white/5 border border-white/[0.07] rounded-xl p-5">
                        <div class="font-title text-xs uppercase tracking-widest text-white/50 mb-2">
                            "Soon, but you can plan it"
                        </div>
                        {case_list(CASES_SOON, "bg-white/[0.07] text-white/50")}
                        <p class="text-sm text-white/50 mt-3">
                            "Rotate the affected keys out, or migrate to a fresh seed, on your own schedule."
                        </p>
                    </div>
                </div>
            </Section>

            // ---------- address reuse ----------
            <Section id="scams" title="Nobody legitimate will ask for your seed">
                <div class="bg-[#ffce6b]/[0.07] border border-[#ffce6b]/30 rounded-xl p-5">
                    <p class="text-sm text-white/80 leading-relaxed mb-3">
                        "Incidents like this bring out people offering to help. Assume every unsolicited offer is
                         a thief. No support agent, no recovery service, no migration tool and no wallet developer
                         ever needs your seed words or your passphrase. Anyone who asks is stealing from you."
                    </p>
                    <p class="text-sm text-white/70 leading-relaxed">
                        "Do not type your words into a website. Do not install a tool someone sent you. Do not
                         accept help over a direct message, and do not trust an email about this even if it looks
                         like it came from the vendor. If you need a hand, ask someone you already knew before
                         today."
                    </p>
                </div>
            </Section>

            <Section id="already-gone" title="First, check the coins are still there">
                <p class="text-sm text-white/75 leading-relaxed mb-3">
                    "Before planning anything, confirm you still have a balance. Look up your address or your
                     wallet on a block explorer, or open a watch-only copy of the wallet. Do not enter your seed
                     anywhere to do this, and use a public key or address only."
                </p>
                <p class="text-sm text-white/75 leading-relaxed">
                    "If the funds are already gone there is nothing to migrate, and nobody can reverse it. Save
                     the transaction IDs and the addresses they went to, because that record is the only thing
                     that is useful later. If the funds are still there, keep reading."
                </p>
            </Section>

            <Section id="address-reuse" title="Reused an address? Move now">
                <div class="bg-[#ffce6b]/[0.07] border border-[#ffce6b]/30 rounded-xl p-5">
                    <p class="text-sm text-white/80 leading-relaxed">
                        "If you have received to the same address more than once, treat yourself as the most
                         urgent case no matter which of the three above describes you. Address reuse hands an
                         attacker a fixed target to watch and to test guessed keys against, and it removes the
                         small amount of cover that fresh addresses give you."
                    </p>
                </div>
            </Section>

            // ---------- entropy table ----------
            <Section id="passphrase" title="What is your passphrase actually worth?">
                <p class="text-sm text-white/60 leading-relaxed mb-4">
                    "Entropy in bits, by what your passphrase is built from and how many units long it is.
                     128 bits is the target. Green is at or above it, amber is close, grey is not enough."
                </p>

                <div class="overflow-x-auto -mx-6 px-6 lg:mx-0 lg:px-0">
                    <table class="w-full text-sm border-collapse">
                        <thead>
                            <tr class="border-b border-white/15">
                                <th class="text-left font-title text-xs uppercase tracking-wider text-[#f7931a] py-2 pr-3">
                                    "Built from"
                                </th>
                                {LENGTHS.iter().map(|n| view! {
                                    <th class="text-right font-title text-xs uppercase tracking-wider text-[#f7931a] py-2 px-2">
                                        {format!("{}", n)}
                                    </th>
                                }).collect::<Vec<_>>()}
                                <th class="text-right font-title text-xs uppercase tracking-wider text-white/40 py-2 pl-2 whitespace-nowrap">
                                    "for 128"
                                </th>
                            </tr>
                        </thead>
                        <tbody>
                            {CHARSETS.iter().map(|(name, per)| {
                                let per = *per;
                                view! {
                                    <tr class="border-b border-white/[0.06]">
                                        <td class="text-white/80 py-2 pr-3">{*name}</td>
                                        {LENGTHS.iter().map(|n| {
                                            let bits = per * (*n as f64);
                                            view! {
                                                <td class=format!("text-right py-2 px-2 tabular-nums {}", bits_class(bits))>
                                                    {format!("{:.0}", bits)}
                                                </td>
                                            }
                                        }).collect::<Vec<_>>()}
                                        <td class="text-right py-2 pl-2 text-white/50 tabular-nums">
                                            {format!("{}", (128.0 / per).ceil() as u32)}
                                        </td>
                                    </tr>
                                }
                            }).collect::<Vec<_>>()}
                        </tbody>
                    </table>
                </div>

                <p class="text-xs text-white/40 mt-3">
                    "Column headings are the number of words or characters in your passphrase."
                </p>

                <div class="bg-white/5 border border-white/10 rounded-xl p-5 mt-5">
                    <div class="font-title text-xs uppercase tracking-widest text-[#f7931a] mb-2">
                        "Read this before you trust the table"
                    </div>
                    <p class="text-sm text-white/75 leading-relaxed mb-3">
                        "Every number above assumes each word or character was chosen at random. A long
                         passphrase you thought up yourself is worth a small fraction of what the table says.
                         A memorable sentence, a quote, a pattern on the keyboard, or a password you have used
                         anywhere else carries almost no entropy at all."
                    </p>
                    <p class="text-sm text-white/75 leading-relaxed">
                        "And reaching 128 bits does not repair the seed. It means the passphrase is the only
                         thing left protecting coins whose underlying key may already be known. That buys you
                         time to migrate carefully. It is not a reason to stay."
                    </p>
                </div>
            </Section>

            // ---------- dice ----------
            <Section id="dice" title="What your dice rolls were worth">
                <p class="text-sm text-white/60 leading-relaxed mb-4">
                    "A fair six-sided die contributes about 2.6 bits per roll, and dice entropy came from you
                     rather than from the device."
                </p>
                <div class="flex flex-wrap gap-2">
                    {[30usize, 50, 75, 99, 100].iter().map(|n| {
                        let bits = DICE_BITS * (*n as f64);
                        view! {
                            <div class="bg-white/5 border border-white/10 rounded-lg px-3.5 py-2">
                                <span class="text-white/80 text-sm">{format!("{} rolls", n)}</span>
                                <span class=format!("text-sm ml-2 tabular-nums {}", bits_class(bits))>
                                    {format!("{:.0} bits", bits)}
                                </span>
                            </div>
                        }
                    }).collect::<Vec<_>>()}
                </div>
                <p class="text-sm text-white/60 leading-relaxed mt-4">
                    "If you are not certain how many rolls you entered, or whether anyone could have seen them,
                     do not count them at all. Treat your wallet as one that needs moving urgently."
                </p>
            </Section>

            // ---------- broadcasting ----------
            <Section id="broadcast" title="Moving funds without being front-run">
                <p class="text-sm text-white/75 leading-relaxed mb-3">
                    "If an attacker may already hold your key, a normal broadcast is a race. They can see your
                     transaction sitting in the mempool and try to replace it with one paying themselves."
                </p>
                <p class="text-sm text-white/75 leading-relaxed mb-4">
                    "Submitting the transaction straight to a miner avoids the race, because it is already in
                     a block by the time anyone else sees it. MARA has now opened its Slipstream service to
                     everyone, so this no longer needs an account or an introduction. If you are in the most
                     urgent group and moving a meaningful amount, take the extra step."
                </p>
                <a
                    class="inline-flex items-center gap-2 px-5 py-2.5 rounded-xl text-sm font-semibold text-[#f7931a] bg-[#f7931a]/10 border border-[#f7931a]/40 hover:bg-[#f7931a]/20 hover:border-[#f7931a]/60 transition-all duration-200"
                    href=SLIPSTREAM_URL
                    target="_blank"
                    rel="noreferrer"
                >
                    "Submit through MARA Slipstream \u{2192}"
                </a>
            </Section>

            // ---------- where to ----------
            <Section id="move-now" title="Where to move it right now">
                <p class="text-sm text-white/75 leading-relaxed mb-4">
                    "If you need to move today, do not wait until you have decided on your permanent setup. Get
                     the coins off the compromised key first. Any of these will generate a fresh wallet in a few
                     minutes and none of them are affected by this flaw:"
                </p>
                <ul class="text-sm text-white/80 leading-relaxed space-y-2 mb-5">
                    <li>
                        <span class="text-white font-medium">"Sparrow"</span>
                        " on desktop, if you are comfortable on a computer."
                    </li>
                    <li>
                        <span class="text-white font-medium">"Blue Wallet"</span>
                        ", "
                        <span class="text-white font-medium">"Blockstream Green"</span>
                        " or "
                        <span class="text-white font-medium">"Cove"</span>
                        " on a phone, if you want this done in the next ten minutes."
                    </li>
                </ul>
                <p class="text-sm text-white/75 leading-relaxed mb-4">
                    "My "
                    <a
                        class="text-[#f7931a] hover:text-[#f4a949] underline underline-offset-2 transition-colors"
                        href="/guides/basic"
                    >
                        "Basic guides"
                    </a>
                    " walk through Sparrow, Blue Wallet and Blockstream Green step by step, including writing the
                     backup down properly. They are unaffected and still online."
                </p>
                <div class="bg-white/5 border border-white/10 rounded-xl p-5 mb-4">
                    <div class="font-title text-xs uppercase tracking-widest text-[#f7931a] mb-2">
                        "Do not make the new seed on the COLDCARD"
                    </div>
                    <p class="text-sm text-white/75 leading-relaxed">
                        "Coinkite's own guidance is to update the firmware and generate a replacement seed on the
                         same device. I would not. Updating fixes new seeds but the device has already lost the
                         benefit of the doubt on the one job that matters most. Generate the new seed somewhere
                         else."
                    </p>
                </div>
                <p class="text-sm text-white/70 leading-relaxed">
                    "A phone wallet is not where large savings belong, and I am not pretending otherwise. But a
                     fresh seed on a phone is safer tonight than a compromised seed on a hardware wallet, and you
                     can move again later once you have decided properly. If the amount is large enough that this
                     makes you uneasy, parking it briefly with a custodian or exchange account you already have
                     is also better than losing it. Neither is a permanent answer, and both beat waiting."
                </p>
            </Section>

            <Section id="where-to-move" title="Where to keep it long term">
                <p class="text-sm text-white/75 leading-relaxed mb-4">
                    "I am not naming a replacement device yet. The lesson of this failure is not that COLDCARD
                     was the wrong brand, it is that trusting any single vendor to get entropy right leaves you
                     with no margin when they do not. Other vendors are being reviewed and so far nothing
                     comparable has turned up elsewhere, which is encouraging, but I would rather wait than
                     send people rushing from one device to another."
                </p>
                <p class="text-sm text-white/75 leading-relaxed mb-4">
                    "What I would hold to instead:"
                </p>
                <ul class="text-sm text-white/75 leading-relaxed space-y-2 mb-5">
                    <li>
                        <span class="text-white font-medium">"Spread the trust. "</span>
                        "For meaningful amounts, use multisig with devices from different manufacturers, so one
                         vendor's bug cannot spend your coins."
                    </li>
                    <li>
                        <span class="text-white font-medium">"Bring your own entropy. "</span>
                        "Dice are the reason some people are unaffected today. Generating a seed yourself, away
                         from the device, removes the vendor from the part that matters most."
                    </li>
                    <li>
                        <span class="text-white font-medium">"Verify before you fund. "</span>
                        "Write the backup down, restore it, confirm the fingerprint matches, and send a small
                         test amount before moving the balance."
                    </li>
                    <li>
                        <span class="text-white font-medium">"Do not rush into a worse setup. "</span>
                        "A panicked migration into something you do not understand is its own way to lose coins."
                    </li>
                </ul>
                <p class="text-sm text-white/60 leading-relaxed">
                    "For specific product opinions, read "
                    <a
                        class="text-[#f7931a] hover:text-[#f4a949] underline underline-offset-2 transition-colors"
                        href=ROB_THREAD_URL
                        target="_blank"
                        rel="noreferrer"
                    >
                        "Rob Hamilton's thread"
                    </a>
                    " and Michael Flaxman's "
                    <a
                        class="text-[#f7931a] hover:text-[#f4a949] underline underline-offset-2 transition-colors"
                        href=FLAXMAN_GUIDE_URL
                        target="_blank"
                        rel="noreferrer"
                    >
                        "multi-vendor multisig guide"
                    </a>
                    ", which pioneered the idea and remains the clearest mental model for a rebuild. One caveat
                     on that one: it recommends the COLDCARD, having been written years before this failure.
                     Take its principles and put a different device in that slot. Weigh both authors'
                     disclosures for yourself."
                </p>
            </Section>

            // ---------- my own guides ----------
            <Section id="my-guides" title="Why my hardware guides are offline">
                <p class="text-sm text-white/75 leading-relaxed">
                    "My Intermediate and Advanced guides were built around the COLDCARD, so they are down
                     rather than left up as advice I no longer stand behind. They will return rebuilt around
                     multiple vendors and your own entropy. The "
                    <a
                        class="text-[#f7931a] hover:text-[#f4a949] underline underline-offset-2 transition-colors"
                        href="/guides/basic"
                    >
                        "Basic guides"
                    </a>
                    " are unaffected and still available."
                </p>
            </Section>

            // ---------- sources ----------
            <Section id="sources" title="Sources">
                <ul class="text-sm space-y-2">
                    <li>
                        <a
                            class="text-[#f7931a] hover:text-[#f4a949] underline underline-offset-2 transition-colors"
                            href=BLOCK_REPORT_URL
                            target="_blank"
                            rel="noreferrer"
                        >
                            "Block Engineering: the technical report that identified the flaw"
                        </a>
                    </li>
                    <li>
                        <a
                            class="text-[#f7931a] hover:text-[#f4a949] underline underline-offset-2 transition-colors"
                            href=COLDCARD_ADVISORY_URL
                            target="_blank"
                            rel="noreferrer"
                        >
                            "Coinkite's own advisory and firmware guidance"
                        </a>
                    </li>
                    <li>
                        <a
                            class="text-[#f7931a] hover:text-[#f4a949] underline underline-offset-2 transition-colors"
                            href=ROB_THREAD_URL
                            target="_blank"
                            rel="noreferrer"
                        >
                            "Rob Hamilton: triage thread and personal recommendations"
                        </a>
                    </li>
                </ul>
                <p class="text-xs text-white/40 mt-4 leading-relaxed">
                    "This page is my reading of a developing situation and is not advice. Verify against the
                     sources above and make your own decisions."
                </p>
            </Section>
        </div>
    }
}
