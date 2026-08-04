//! Guide content, v2 (structured "Refined" step model).
//!
//! Unlike the v1 markdown-blob guides (src/faqs/<dir>/*.md rendered by Stepper),
//! a v2 guide is fully typed, compile-time data. The renderer is
//! `src/extras/stepper_v2.rs` (StepperV2). A wallet opts into v2 via
//! `find_guide_v2(wallet_id)`; if it returns Some, the wallet page renders the
//! wizard instead of the old download + Stepper layout. Old guides are untouched.
//!
//! No Leptos here on purpose: this file is pure data so it stays portable and
//! trivially testable. Inline `**bold**` / `[text](url)` in copy is parsed by
//! the renderer (see stepper_v2::inline), never via inner_html.

/// Which device frame wraps a step's screenshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frame {
    /// Phone bezel (mobile wallet screenshots).
    Phone,
    /// Desktop window chrome (coordinator screenshots, e.g. Sparrow).
    Desktop,
}

/// A numbered indicator pinned over a screenshot. `n` matches the numbered action
/// on the left (pin 1 = action 1, ...). `x`/`y` are percentages of the framed image
/// (0-100); the frame matches the image's aspect so they map 1:1. CONVENTION: place
/// the pin just to the LEFT of the control it highlights (in the margin), vertically
/// centered on it, so it never covers the label. `label` is a hidden a11y hint.
#[derive(Debug, Clone, Copy)]
pub struct Pin {
    pub n: u8,
    pub x: f32,
    pub y: f32,
    pub label: &'static str,
}

/// One screenshot inside a device frame: the image, its pins, a caption, and the
/// intrinsic pixel size (so the frame matches aspect and pins map 1:1).
#[derive(Debug, Clone, Copy)]
pub struct Shot {
    /// Served path, e.g. "/guide-images/cove/cove-receive-01-address.png".
    pub image: &'static str,
    pub alt: &'static str,
    pub caption: &'static str,
    pub img_w: u32,
    pub img_h: u32,
    pub pins: &'static [Pin],
}

/// A framed device for a step. One or more shots; multiple shots render as a
/// carousel inside the frame. Empty `shots` => the step renders single-column.
#[derive(Debug, Clone, Copy)]
pub struct Device {
    pub frame: Frame,
    pub shots: &'static [Shot],
}

/// One guide step.
#[derive(Debug, Clone, Copy)]
pub struct Step {
    pub title: &'static str,
    /// One-line objective, shown in the goal banner.
    pub goal: &'static str,
    /// Short, bold-verb actions. Support `**bold**` and `[text](url)`.
    pub actions: &'static [&'static str],
    /// Optional warning callout.
    pub flag: Option<&'static str>,
    /// Optional "why this matters" disclosure: (summary, body).
    pub why: Option<(&'static str, &'static str)>,
    /// "You will need" chips.
    pub needs: &'static [&'static str],
    /// Whether to surface the backup-sheet CTA on this step.
    pub backup_cta: bool,
    pub device: Device,
}

/// The guide's opening panel.
#[derive(Debug, Clone, Copy)]
pub struct Intro {
    pub title: &'static str,
    pub lede: &'static str,
    /// Meta chips, e.g. "5 steps", "~15 min".
    pub chips: &'static [&'static str],
    /// "What you will have at the end" checklist.
    pub outcomes: &'static [&'static str],
    pub backup_cta: bool,
}

/// The guide's closing panel.
#[derive(Debug, Clone, Copy)]
pub struct Completion {
    pub title: &'static str,
    pub lede: &'static str,
    /// Optional next-tier link: (label, href).
    pub next_tier: Option<(&'static str, &'static str)>,
    pub backup_cta: bool,
}

/// A full v2 guide: intro, steps, completion.
#[derive(Debug, Clone, Copy)]
pub struct GuideV2 {
    /// Small kicker, e.g. "Basic · Cove".
    pub eyebrow: &'static str,
    pub intro: Intro,
    pub steps: &'static [Step],
    pub completion: Completion,
}

/// Look up a v2 guide by wallet id. Some => render StepperV2, None => v1 Stepper.
pub fn find_guide_v2(wallet_id: &str) -> Option<&'static GuideV2> {
    match wallet_id {
        "cove" => Some(&COVE_GUIDE),
        "bull" => Some(&BULL_GUIDE),
        "nunchuk" => Some(&NUNCHUK_GUIDE),
        "sparrow" => Some(&SPARROW_GUIDE),
        _ => None,
    }
}

/// One part of a multi-part level. Levels that are not a "pick a wallet" choice
/// (Intermediate, later Advanced) are split into parts so the level page can offer
/// the same card picker the wallet levels get, instead of dropping the reader
/// straight into a long wizard. Each part is its own StepperV2 guide, served at
/// `/guides/<level>/<platform>/<part id>`.
pub struct LevelPart {
    pub id: &'static str,
    pub name: &'static str,
    pub tagline: &'static str,
    /// Short "what you get" chips, same idea as WalletDef::highlights.
    pub highlights: &'static [&'static str],
    pub guide: &'static GuideV2,
}

pub static INTERMEDIATE_PARTS: &[LevelPart] = &[
    LevelPart {
        id: "hardware",
        name: "Part 1: Hardware wallet",
        tagline: "Generate your keys offline on a Coldcard, protect them with a passphrase, back them up in steel, and drive it all from Sparrow.",
        highlights: &["Coldcard, fully air-gapped", "Passphrase protected", "Steel backup"],
        guide: &INTERMEDIATE_HARDWARE_GUIDE,
    },
    LevelPart {
        id: "node",
        name: "Part 2: Your own node",
        tagline: "Run Bitcoin yourself and point Sparrow at it, so no third party sees your addresses or tells you what is true.",
        highlights: &["Validate every block", "Private by default", "Start9, MyNode or RaspiBlitz"],
        guide: &INTERMEDIATE_NODE_GUIDE,
    },
];

pub static ADVANCED_PARTS: &[LevelPart] = &[
    LevelPart {
        id: "multisig",
        name: "Part 1: Build the multisig",
        tagline: "Set up three Coldcards, combine them into a 2-of-3 with the air-gapped tool, and coordinate it from Sparrow.",
        highlights: &["2-of-3, no single point of failure", "Fully air-gapped setup", "Output descriptor backed up"],
        guide: &ADVANCED_MULTISIG_GUIDE,
    },
    LevelPart {
        id: "spending",
        name: "Part 2: Receive and spend",
        tagline: "Take funds in, then walk a transaction out to two devices and back so the round trip is familiar before it matters.",
        highlights: &["Receive to the quorum", "Sign a PSBT on two devices", "Broadcast from your own node"],
        guide: &ADVANCED_SPENDING_GUIDE,
    },
    LevelPart {
        id: "hardening",
        name: "Part 3: Harden it further",
        tagline: "Optional extras for specific threat models: decoy wallets, SeedXOR, and automated signing. Skip what does not fit.",
        highlights: &["Duress and decoy wallets", "SeedXOR done properly", "Optional, adds complexity"],
        guide: &ADVANCED_HARDENING_GUIDE,
    },
];

/// Parts for a level, empty when the level is a wallet-picker level.
pub fn parts_for_level(level_id: &str) -> &'static [LevelPart] {
    match level_id {
        "intermediate" => INTERMEDIATE_PARTS,
        "advanced" => ADVANCED_PARTS,
        _ => &[],
    }
}

/// Look up one part of a level by its id (the last URL segment).
pub fn find_level_part(
    level_id: &str,
    part_id: &str,
) -> Option<&'static LevelPart> {
    parts_for_level(level_id).iter().find(|p| p.id == part_id)
}

/// Sentinel for a step with no screenshot: the renderer shows a single centered
/// column (no device frame) when `shots` is empty.
const NO_DEVICE: Device = Device {
    frame: Frame::Desktop,
    shots: &[],
};

// =============================================================================
// COVE (Basic) — the simplest mobile path, and the first card in the picker.
// Screenshots live in assets/guide-images/cove/ (served at /guide-images/cove/).
// All Cove screenshots are 1080 x 2424.
// =============================================================================

const COVE_W: u32 = 1080;
const COVE_H: u32 = 2424;

pub static COVE_GUIDE: GuideV2 = GuideV2 {
    eyebrow: "Basic · Cove",
    intro: Intro {
        title: "Set up Cove",
        lede: "A simple, self-custodied wallet. Create a wallet, write down your recovery words and learn to receive and send bitcoin.",
        chips: &["5 steps", "about 15 min", "best for small amounts"],
        outcomes: &[
            "Your own Bitcoin wallet, with the keys held by you",
            "Your recovery words written down safely",
            "The confidence to receive and send bitcoin",
        ],
        backup_cta: true,
    },
    steps: &[
        // 1 · Create
        Step {
            title: "Create your wallet",
            goal: "Create a Cove wallet with the keys held on your phone.",
            actions: &[
                "Open Cove, read the terms, and tap **Agree and Continue**.",
                "Choose **On This Device** so the keys stay on your phone.",
                "Tap **Create new wallet**.",
                "Pick **12 or 24 words** for your recovery phrase (I recommend **24**).",
            ],
            flag: None,
            why: Some((
                "Hot wallet vs hardware wallet",
                "A hot wallet keeps your keys on the phone, which is ideal for a small everyday spending stack. When you are ready to protect larger savings, the Intermediate guide moves your keys onto a dedicated hardware device.",
            )),
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Phone,
                shots: &[
                    Shot {
                        image: "/guide-images/cove/cove-onboarding-00-terms.png",
                        alt: "Cove, agree to the terms and conditions",
                        caption: "Cove, terms",
                        img_w: COVE_W,
                        img_h: COVE_H,
                        pins: &[Pin { n: 1, x: 12.0, y: 85.0, label: "Agree and Continue" }],
                    },
                    Shot {
                        image: "/guide-images/cove/cove-onboarding-02-secure-choice.png",
                        alt: "Cove, choose how to secure your Bitcoin: Hardware Wallet or On This Device",
                        caption: "Cove, secure choice",
                        img_w: COVE_W,
                        img_h: COVE_H,
                        pins: &[Pin { n: 2, x: 56.0, y: 92.0, label: "Choose On This Device" }],
                    },
                    Shot {
                        image: "/guide-images/cove/cove-onboarding-01-have-wallet.png",
                        alt: "Cove, do you already have a wallet? Create new wallet or import",
                        caption: "Cove, create a new wallet",
                        img_w: COVE_W,
                        img_h: COVE_H,
                        pins: &[Pin { n: 3, x: 12.0, y: 88.0, label: "Tap Create new wallet" }],
                    },
                    Shot {
                        image: "/guide-images/cove/cove-onboarding-03-select-word-count.png",
                        alt: "Cove, select the number of recovery words: 12 or 24",
                        caption: "Cove, select word count",
                        img_w: COVE_W,
                        img_h: COVE_H,
                        pins: &[Pin { n: 4, x: 8.0, y: 86.0, label: "Pick 12 or 24 words" }],
                    },
                ],
            },
        },
        // 2 · Back up recovery words
        Step {
            title: "Write down your recovery words",
            goal: "Save your bitcoin wallet's recovery words. Used to recover your bitcoin in the event of a lost phone.",
            actions: &[
                "Cove shows your **recovery words** in order. Write them down on [paper](/downloads/seed-backup-sheet.html), then tap **Next**.",
                "Write down the rest of the words, double-check every spelling, then tap **Save Wallet**.",
                "Cove asks you to verify each word in turn. Tap the **correct word** each time.",
                "Once verified, tap **Go To Wallet** to finish.",
                "Congrats, your wallet has been created. Next, let's learn to receive some bitcoin.",
            ],
            flag: Some("Never take a photo of these words or type them into any app. Anyone who reads them can take your bitcoin. Paper only."),
            why: Some((
                "Why write them on paper",
                "Your recovery words are the wallet. Anything digital (a screenshot, a note, a cloud backup) can be reached by an attacker. A hand-written copy kept offline cannot.",
            )),
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Phone,
                shots: &[
                    Shot {
                        image: "/guide-images/cove/cove-backup-01-words-1.png",
                        alt: "Cove, recovery words 1 to 12",
                        caption: "Cove, recovery words (1 of 2)",
                        img_w: COVE_W,
                        img_h: COVE_H,
                        pins: &[Pin { n: 1, x: 9.0, y: 92.0, label: "Write them down, then tap Next" }],
                    },
                    Shot {
                        image: "/guide-images/cove/cove-backup-02-words-2.png",
                        alt: "Cove, recovery words 13 to 24",
                        caption: "Cove, recovery words (2 of 2)",
                        img_w: COVE_W,
                        img_h: COVE_H,
                        pins: &[Pin { n: 2, x: 9.0, y: 92.0, label: "Tap Save Wallet" }],
                    },
                    Shot {
                        image: "/guide-images/cove/cove-backup-03-verify.png",
                        alt: "Cove, verify recovery words by selecting the requested word",
                        caption: "Cove, verify recovery words",
                        img_w: COVE_W,
                        img_h: COVE_H,
                        pins: &[Pin { n: 3, x: 8.0, y: 43.0, label: "Tap the correct word" }],
                    },
                    Shot {
                        image: "/guide-images/cove/cove-backup-04-all-set.png",
                        alt: "Cove, backup verified, you are all set",
                        caption: "Cove, all set",
                        img_w: COVE_W,
                        img_h: COVE_H,
                        pins: &[Pin { n: 4, x: 9.0, y: 92.0, label: "Tap Go To Wallet" }],
                    },
                    Shot {
                        image: "/guide-images/cove/cove-home-01-empty.png",
                        alt: "Cove, empty wallet home showing 0 BTC",
                        caption: "Cove, your wallet home",
                        img_w: COVE_W,
                        img_h: COVE_H,
                        pins: &[Pin { n: 5, x: 42.0, y: 17.0, label: "Your balance, 0 BTC for now" }],
                    },
                ],
            },
        },
        // 3 · Receive
        Step {
            title: "Receive bitcoin",
            goal: "Get an address so someone can send bitcoin to your wallet.",
            actions: &[
                "Open your wallet and tap **Receive**.",
                "Cove shows a **QR code** and an address that starts with **bc1**. Let the sender scan the QR, or tap **Copy Address** to share it.",
                "When the payment is sent, it appears on your home screen as **Receiving**.",
                "Tap it to watch the **pending** transaction while it waits for a block.",
                "Once it confirms, it shows as **Received** and the confirmation count climbs.",
            ],
            flag: None,
            why: Some((
                "When is it really mine?",
                "A payment must be included in a block to confirm. It first shows as pending at zero confirmations, and each new block adds one, about 10 minutes apart on average. For low-value transactions, 1 to 3 confirmations is sufficient; for larger ones, I recommend waiting for up to 6.",
            )),
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Phone,
                shots: &[
                    Shot {
                        image: "/guide-images/cove/cove-home-01-empty.png",
                        alt: "Cove, wallet home with the Receive button",
                        caption: "Cove, tap Receive",
                        img_w: COVE_W,
                        img_h: COVE_H,
                        pins: &[Pin { n: 1, x: 50.0, y: 25.0, label: "Tap Receive" }],
                    },
                    Shot {
                        image: "/guide-images/cove/cove-receive-01-address.png",
                        alt: "Cove, receive address as a QR code with a Copy Address button",
                        caption: "Cove, receive screen",
                        img_w: COVE_W,
                        img_h: COVE_H,
                        pins: &[Pin { n: 2, x: 8.0, y: 86.0, label: "Copy Address, or let the sender scan the QR" }],
                    },
                    Shot {
                        image: "/guide-images/cove/cove-home-02-receiving.png",
                        alt: "Cove, home screen showing an incoming transaction as Receiving",
                        caption: "Cove, incoming payment",
                        img_w: COVE_W,
                        img_h: COVE_H,
                        pins: &[Pin { n: 3, x: 50.0, y: 38.0, label: "The incoming payment shows as Receiving" }],
                    },
                    Shot {
                        image: "/guide-images/cove/cove-receive-04-pending-details.png",
                        alt: "Cove, transaction pending details",
                        caption: "Cove, pending details",
                        img_w: COVE_W,
                        img_h: COVE_H,
                        pins: &[Pin { n: 4, x: 9.0, y: 27.0, label: "Pending until it lands in a block" }],
                    },
                    Shot {
                        image: "/guide-images/cove/cove-receive-05-received-details.png",
                        alt: "Cove, received transaction with confirmation count",
                        caption: "Cove, received",
                        img_w: COVE_W,
                        img_h: COVE_H,
                        pins: &[],
                    },
                ],
            },
        },
        // 4 · Send
        Step {
            title: "Send bitcoin",
            goal: "Send bitcoin from your wallet to someone else's bitcoin address.",
            actions: &[
                "On your wallet home, tap **Send**.",
                "Enter the **amount** to send.",
                "Paste or scan the recipient's **address**.",
                "Set the **network fee** to match how urgent the payment is, then tap **Next**.",
                "Review the amount, address and fee, then **swipe to send**.",
                "Cove broadcasts it and shows **Transaction Pending** while it waits for a block.",
                "Back on home, your **balance updates** and the payment appears in your history as **Sending**.",
            ],
            flag: Some("Always re-read the address before sending. Bitcoin transactions cannot be reversed."),
            why: Some((
                "How network fees work",
                "Every transaction pays a fee to the miners who include it in a block. Block space is limited, so fees rise and fall with demand. A higher fee usually confirms within a block or two; a lower fee still gets there, it just waits longer for a quiet moment. If your payment is not urgent, tap Change speed and pick a cheaper rate to save sats; if it needs to land fast, choose a higher one.",
            )),
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Phone,
                shots: &[
                    Shot {
                        image: "/guide-images/cove/cove-home-02-receiving.png",
                        alt: "Cove, wallet home with the Send button",
                        caption: "Cove, tap Send",
                        img_w: COVE_W,
                        img_h: COVE_H,
                        pins: &[Pin { n: 1, x: 9.0, y: 25.0, label: "Tap Send" }],
                    },
                    Shot {
                        image: "/guide-images/cove/cove-send-02-compose-filled.png",
                        alt: "Cove, send compose screen with amount, address and network fee",
                        caption: "Cove, compose the payment",
                        img_w: COVE_W,
                        img_h: COVE_H,
                        pins: &[
                            Pin { n: 2, x: 6.0, y: 41.0, label: "Enter the amount" },
                            Pin { n: 3, x: 6.0, y: 60.0, label: "Paste or scan the address" },
                            Pin { n: 4, x: 6.0, y: 76.0, label: "Set the network fee" },
                        ],
                    },
                    Shot {
                        image: "/guide-images/cove/cove-send-04-confirm-swipe.png",
                        alt: "Cove, confirm the payment by swiping to send",
                        caption: "Cove, review and swipe to send",
                        img_w: COVE_W,
                        img_h: COVE_H,
                        pins: &[Pin { n: 5, x: 6.0, y: 91.0, label: "Swipe to send" }],
                    },
                    Shot {
                        image: "/guide-images/cove/cove-send-06-pending.png",
                        alt: "Cove, transaction pending after sending",
                        caption: "Cove, sending",
                        img_w: COVE_W,
                        img_h: COVE_H,
                        pins: &[Pin { n: 6, x: 9.0, y: 40.0, label: "Pending until it lands in a block" }],
                    },
                    Shot {
                        image: "/guide-images/cove/cove-home-03-after-send.png",
                        alt: "Cove, home screen after sending with updated balance",
                        caption: "Cove, balance updated",
                        img_w: COVE_W,
                        img_h: COVE_H,
                        pins: &[Pin { n: 7, x: 4.0, y: 43.0, label: "The send appears as Sending" }],
                    },
                ],
            },
        },
        // 5 · Recover
        Step {
            title: "If you lose your phone",
            goal: "Know how to recover your bitcoin onto a new device using your written words.",
            actions: &[
                "Install Cove (or any other **BIP39** wallet, meaning any wallet that restores from recovery words) on a new phone.",
                "Open Cove and choose **On This Device** when asked how to secure your bitcoin.",
                "On the next screen, tap **Import existing wallet**.",
                "Pick how many words your phrase has (**24** if you followed this guide).",
                "Type your words **in order** across both pages, then tap **Import wallet**.",
                "Your wallet and full history are restored. Give it a minute to sync, then your balance and past transactions appear.",
            ],
            flag: Some("Treat the lost phone as compromised. After recovering, create a brand new wallet (a fresh set of keys with its own new recovery words, not the same phrase again) and move all funds to it. Anyone who ends up with the old phone or its written words could otherwise take your bitcoin."),
            why: None,
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Phone,
                shots: &[
                    Shot {
                        image: "/guide-images/cove/cove-onboarding-02-secure-choice.png",
                        alt: "Cove, choose how to secure your bitcoin: Hardware Wallet or On This Device",
                        caption: "Cove, secure choice",
                        img_w: COVE_W,
                        img_h: COVE_H,
                        pins: &[Pin { n: 2, x: 54.0, y: 92.0, label: "Choose On This Device" }],
                    },
                    Shot {
                        image: "/guide-images/cove/cove-onboarding-01-have-wallet.png",
                        alt: "Cove, do you already have a wallet? Create new or import existing",
                        caption: "Cove, import existing wallet",
                        img_w: COVE_W,
                        img_h: COVE_H,
                        pins: &[Pin { n: 3, x: 28.0, y: 94.0, label: "Tap Import existing wallet" }],
                    },
                    Shot {
                        image: "/guide-images/cove/cove-recover-01-select-words.png",
                        alt: "Cove, import, select the number of recovery words",
                        caption: "Cove, select word count",
                        img_w: COVE_W,
                        img_h: COVE_H,
                        pins: &[Pin { n: 4, x: 4.0, y: 92.0, label: "Pick the number of words in your phrase" }],
                    },
                    Shot {
                        image: "/guide-images/cove/cove-recover-02-enter-words-page1.png",
                        alt: "Cove, import wallet, enter your recovery words",
                        caption: "Cove, enter your words",
                        img_w: COVE_W,
                        img_h: COVE_H,
                        pins: &[Pin { n: 5, x: 4.0, y: 34.0, label: "Type your words in order" }],
                    },
                    Shot {
                        image: "/guide-images/cove/cove-recover-03-home-page-recovered.png",
                        alt: "Cove, home screen with the wallet and history restored",
                        caption: "Cove, wallet restored",
                        img_w: COVE_W,
                        img_h: COVE_H,
                        pins: &[Pin { n: 6, x: 4.0, y: 40.0, label: "Balance and history restored after a short sync" }],
                    },
                ],
            },
        },
    ],
    completion: Completion {
        title: "You are self-custodied",
        lede: "Your bitcoin is in your hands now. Keep your recovery words safe, and when your stack grows, level up.",
        next_tier: Some(("Level up to Intermediate", "/guides/intermediate/desktop")),
        backup_cta: false,
    },
};
pub static SPARROW_GUIDE: GuideV2 = GuideV2 {
    eyebrow: "Basic · Sparrow",
    intro: Intro {
        title: "Set up Sparrow",
        lede: "A single-signature desktop wallet secured with a passphrase. It is sturdier than a phone wallet and a solid base to grow from. You will verify the app, create a wallet, back up your keys, and learn to receive, send and recover.",
        chips: &["6 steps", "about 40 min", "desktop, more secure"],
        outcomes: &[
            "A single-sig Sparrow wallet, with the keys held by you",
            "Your 24 words and passphrase written down and stored separately",
            "The confidence to receive, send and recover on desktop",
        ],
        backup_cta: true,
    },
    steps: &[
        // 1 · Download & verify
        Step {
            title: "Download and verify Sparrow",
            goal: "Get Sparrow from the official site and confirm the download is genuine before installing.",
            actions: &[
                "Download Sparrow for your operating system from [sparrowwallet.com](https://sparrowwallet.com/download/).",
                "Verify the download against its signature by following the verification steps on the [Sparrow download page](https://sparrowwallet.com/download/).",
                "Install Sparrow and open it.",
            ],
            flag: Some("Only ever download Sparrow from sparrowwallet.com. Verifying the signature confirms nobody tampered with the file on its way to you."),
            why: Some((
                "Why verify the binary",
                "A wallet handles your keys, so you want to be certain the file you installed is exactly what the developers published. Verifying the signature catches a corrupted or malicious download before it ever touches your bitcoin.",
            )),
            needs: &[],
            backup_cta: false,
            device: NO_DEVICE,
        },
        // 2 · Create the wallet (configure + generate)
        Step {
            title: "Create your wallet",
            goal: "Create a single-sig software wallet and generate a fresh 24-word seed.",
            actions: &[
                "Open Sparrow and read the four intro screens, clicking **Next** through them.",
                "On the last one, click **Later or Offline Mode**. A public server is fine to start; a later guide moves you onto your own node.",
                "From the **File** menu choose **New Wallet**, give it a name, and click **Create Wallet**.",
                "Under Keystores, click **New or Imported Software Wallet**, then set the length to **Use 24 Words**.",
                "Tick **Use passphrase?**, then click **Generate New** to create your seed.",
            ],
            flag: None,
            why: Some((
                "Why add a passphrase",
                "The passphrase is an extra secret you add on top of your recovery words, not one of the words itself. If someone finds your written words, they still cannot reach your bitcoin without the passphrase. Kept apart, neither piece is enough on its own.",
            )),
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Desktop,
                shots: &[
                    Shot {
                        image: "/guide-images/sparrow/sparrow-onboarding-1.png",
                        alt: "Sparrow welcome and introduction screen",
                        caption: "Read the four intro screens, clicking Next",
                        img_w: 599,
                        img_h: 564,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow-onboarding-4.png",
                        alt: "Sparrow connection intro, Later or Offline Mode button",
                        caption: "On the last screen, click Later or Offline Mode",
                        img_w: 598,
                        img_h: 555,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow-onboarding-5-first-wallet.png",
                        alt: "Sparrow empty state, File menu New Wallet",
                        caption: "File menu, New Wallet",
                        img_w: 1068,
                        img_h: 810,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow-onboarding-6-first-wallet-name.png",
                        alt: "Sparrow, name the wallet and click Create Wallet",
                        caption: "Name it, then Create Wallet",
                        img_w: 1072,
                        img_h: 808,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow-onboarding-7-first-wallet-new.png",
                        alt: "Sparrow keystores, New or Imported Software Wallet",
                        caption: "Choose New or Imported Software Wallet",
                        img_w: 1069,
                        img_h: 812,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow-onboarding-8-first-wallet-seed-length.png",
                        alt: "Sparrow, choose the mnemonic length, Use 24 Words",
                        caption: "Set the length to Use 24 Words",
                        img_w: 1072,
                        img_h: 806,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow-onboarding-9-first-wallet-seed-generation.png",
                        alt: "Sparrow, tick Use passphrase then Generate New",
                        caption: "Tick Use passphrase, then Generate New",
                        img_w: 1068,
                        img_h: 813,
                        pins: &[],
                    },
                ],
            },
        },
        // 3 · Back up recovery words + finalize
        Step {
            title: "Back up your recovery words",
            goal: "Save your 24 words and passphrase on paper, then finish creating the wallet.",
            actions: &[
                "Sparrow shows your **24 words**. Write them down in order on [paper](/downloads/seed-backup-sheet.html), and write your **passphrase** down separately.",
                "Click **Confirm Backup**, re-enter the words when asked, then click **Create Keystore**.",
                "Click **Import Keystore**, then re-enter your passphrase to confirm.",
                "**Note the master fingerprint** shown, then click **OK**.",
                "Click **Apply** to save the wallet, then set an **optional** wallet password (it encrypts the file on your computer and is not your passphrase), or click **No Password**.",
                "Your wallet opens and starts loading its history.",
            ],
            flag: Some("Store the 24 words and the passphrase on paper, in two separate places. You need both to recover your bitcoin, and neither one alone is enough. Record the master fingerprint too, so you can confirm a correct recovery later."),
            why: None,
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Desktop,
                shots: &[
                    Shot {
                        image: "/guide-images/sparrow/sparrow-onboarding-10-first-wallet-seed-backup.png",
                        alt: "Sparrow shows the 24 words and passphrase, Confirm Backup",
                        caption: "Write down the 24 words and passphrase, then Confirm Backup",
                        img_w: 1068,
                        img_h: 812,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow-onboarding-11-first-wallet-seed-confirm.png",
                        alt: "Sparrow, re-enter the words, Create Keystore",
                        caption: "Re-enter the words, then Create Keystore",
                        img_w: 1070,
                        img_h: 805,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow-onboarding-12-first-wallet-seed-import.png",
                        alt: "Sparrow, Import Keystore",
                        caption: "Click Import Keystore",
                        img_w: 1069,
                        img_h: 802,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow-onboarding-13-first-wallet-seed-reenterpassphrase.png",
                        alt: "Sparrow, re-enter passphrase and note the master fingerprint",
                        caption: "Re-enter the passphrase, note the master fingerprint, then OK",
                        img_w: 1072,
                        img_h: 807,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow-onboarding-14-first-wallet-seed-apply.png",
                        alt: "Sparrow, Apply to save the wallet",
                        caption: "Click Apply to save the wallet",
                        img_w: 1070,
                        img_h: 804,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow-onboarding-15-first-wallet-walletpassword.png",
                        alt: "Sparrow, set a wallet password or No Password",
                        caption: "Set a wallet password, or click No Password",
                        img_w: 1074,
                        img_h: 807,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow-onboarding-17-first-wallet-final.png",
                        alt: "Sparrow, the wallet open and loading its history",
                        caption: "Your wallet, loading its history",
                        img_w: 1072,
                        img_h: 807,
                        pins: &[],
                    },
                ],
            },
        },
        // 4 · Receive
        Step {
            title: "Receive bitcoin",
            goal: "Get a receive address so someone can send bitcoin to your wallet.",
            actions: &[
                "Open your wallet and click the **Receive** tab on the left.",
                "Copy the **address** shown, and optionally add a **Label** to remember where the funds came from.",
                "Share the address with the sender. When it arrives, it appears under the **Transactions** tab.",
                "Sparrow gives you a fresh address each time. Never reuse one; click **Get New Address** if unsure.",
            ],
            flag: None,
            why: Some((
                "Why a new address each time",
                "Reusing an address links your payments together on the public timechain, which hurts your privacy. A fresh address for each receive keeps them separate.",
            )),
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Desktop,
                shots: &[
                    Shot {
                        image: "/guide-images/sparrow/sparrow_basic_wallet.png",
                        alt: "Sparrow, the Receive tab on the left panel",
                        caption: "Click the Receive tab",
                        img_w: 1028,
                        img_h: 771,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow_basic_receive_address.png",
                        alt: "Sparrow, a receive address with a label field",
                        caption: "Copy the address, optionally add a label",
                        img_w: 1027,
                        img_h: 766,
                        pins: &[],
                    },
                ],
            },
        },
        // 5 · Send
        Step {
            title: "Send bitcoin",
            goal: "Send bitcoin from your wallet to another bitcoin address.",
            actions: &[
                "Click the **Send** tab. Paste the recipient **address** into **Pay to**, add an optional **Label**, and enter the **Amount**.",
                "Set the **fee** (Sparrow suggests one; check [mempool.space](https://mempool.space/) and raise it if urgent or lower it if you can wait), then click **Create Transaction**.",
                "Review the details, then click **Finalize Transaction for Signing**.",
                "Click **Sign**, then **Broadcast Transaction** to send it.",
                "Open the **Transactions** tab to watch it confirm.",
            ],
            flag: Some("Triple-check the recipient address before you broadcast. Bitcoin transactions cannot be reversed."),
            why: Some((
                "How network fees work",
                "Every transaction pays a fee to the miners who include it in a block. Block space is limited, so fees rise and fall with demand. A higher fee usually confirms within a block or two; a lower fee still gets there, it just waits longer. For low-value transactions 1 to 3 confirmations is enough; for larger ones, wait for up to 6.",
            )),
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Desktop,
                shots: &[
                    Shot {
                        image: "/guide-images/sparrow/sparrow_wallet_send_details.png",
                        alt: "Sparrow, send tab with pay-to address, label, amount and fee",
                        caption: "Compose: address, amount, fee, then Create Transaction",
                        img_w: 1027,
                        img_h: 764,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow_wallet_send_finalization.png",
                        alt: "Sparrow, finalize transaction for signing",
                        caption: "Finalize Transaction for Signing",
                        img_w: 1026,
                        img_h: 767,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow_wallet_send_signing.png",
                        alt: "Sparrow, sign the transaction",
                        caption: "Sign",
                        img_w: 1031,
                        img_h: 764,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow_wallet_send_broadcasting.png",
                        alt: "Sparrow, broadcast the transaction",
                        caption: "Broadcast Transaction",
                        img_w: 1031,
                        img_h: 762,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow_wallet_send_pending.png",
                        alt: "Sparrow, transactions tab showing the pending send",
                        caption: "Watch it confirm under Transactions",
                        img_w: 1029,
                        img_h: 768,
                        pins: &[],
                    },
                ],
            },
        },
        // 6 · Recover
        Step {
            title: "If you lose your device",
            goal: "Restore your wallet onto a new computer using your recovery words and passphrase.",
            actions: &[
                "Install Sparrow on the new computer, then click **File**, then **Import Wallet**.",
                "Set the first dropdown to the number of words in your phrase (**24** if you followed this guide).",
                "Enter your **words in order**, add your **passphrase**, then click **Discover Wallet** (this can take a minute).",
                "Give the wallet a name (the optional password is unrelated to your passphrase).",
                "Your balance and full history reappear once discovery finishes.",
            ],
            flag: Some("Treat the lost device as compromised. After recovering, create a brand new wallet (fresh recovery words and a new passphrase) and move all funds to it. Anyone who ends up with the old device or its written secrets could otherwise take your bitcoin."),
            why: Some((
                "Recovered but no transactions?",
                "Almost always this means a wrong word, the wrong word order, or the wrong passphrase; any of those silently builds a different, empty wallet. Re-check each word and your passphrase exactly as first written. Your recorded master fingerprint confirms when they match.",
            )),
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Desktop,
                shots: &[
                    Shot {
                        image: "/guide-images/sparrow/sparrow_wallet_recovery_import.png",
                        alt: "Sparrow, File menu, Import Wallet",
                        caption: "File menu, Import Wallet",
                        img_w: 1027,
                        img_h: 775,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow-wallet-recovery-2.png",
                        alt: "Sparrow, import wallet, set Mnemonic Words (BIP39) to Use 24 Words",
                        caption: "For Mnemonic Words (BIP39), choose your word count (Use 24 Words)",
                        img_w: 1070,
                        img_h: 803,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow-wallet-recovery-3.png",
                        alt: "Sparrow, enter the words and passphrase, then discover wallet",
                        caption: "Enter your words and passphrase, then Discover Wallet",
                        img_w: 1071,
                        img_h: 807,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow_wallet_recovery_name.png",
                        alt: "Sparrow, name the recovered wallet",
                        caption: "Name the wallet",
                        img_w: 1030,
                        img_h: 773,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow_wallet_recovery_complete.png",
                        alt: "Sparrow, recovered wallet with balance and history restored",
                        caption: "Balance and history restored",
                        img_w: 1106,
                        img_h: 849,
                        pins: &[],
                    },
                ],
            },
        },
    ],
    completion: Completion {
        title: "You are self-custodied on desktop",
        lede: "Your bitcoin sits behind your own keys and a passphrase now. Keep both backups safe and separate, and when your stack grows, step up to a hardware wallet.",
        next_tier: Some(("Level up to Intermediate", "/guides/intermediate/desktop")),
        backup_cta: false,
    },
};

// =============================================================================
// INTERMEDIATE (level guide) — Coldcard + Sparrow + your own node. Desktop path,
// one guide for all OSes. Content from the v1 hardware_wallet_setup and node_setup
// markdown, reviewed 2026-07-24 to restore what the first condensation dropped
// (encrypted microSD backups, the backup checklist, tamper-bag checks, the node
// details and hardware advice). 11 steps: 1-9 Coldcard, 10 Sparrow, 11 node.
// Most steps have no screenshot (NO_DEVICE => single column); the three that do use
// Frame::Desktop => the stacked full-width layout. Buy/docs links live inline in the
// actions via the [text](url) parser.
// =============================================================================

pub static INTERMEDIATE_HARDWARE_GUIDE: GuideV2 = GuideV2 {
    eyebrow: "Intermediate · Hardware wallet",
    intro: Intro {
        title: "Hardware wallet + Sparrow",
        lede: "Real self-custody starts here. You will generate your keys on a dedicated hardware device (never a phone), protect them with a passphrase, back them up in steel, and connect it all to Sparrow on your desktop.",
        chips: &["10 steps", "a few evenings", "part 1 of intermediate"],
        outcomes: &[
            "Keys generated on a Coldcard, fully offline",
            "A passphrase-protected wallet only you can open",
            "A steel backup that survives fire and water",
            "Sparrow watching and spending, keys never online",
        ],
        backup_cta: true,
    },
    steps: &[
        // 1 · Gather your gear
        Step {
            title: "Gather your gear",
            goal: "Get the hardware in hand before you start.",
            actions: &[
                "Buy a **[Coldcard MK4 bundle](https://store.coinkite.com/store/bundle-mk4-basic)** (about $220, it includes two microSD cards).",
                "Add a **[Seedplate](https://store.coinkite.com/store/seedplate)** and a **[center punch](https://store.coinkite.com/store/drillpunch)** for a steel backup, plus a set of **casino dice** for your own entropy.",
                "Get a way to power the Coldcard offline: **[Coldpower](https://store.coinkite.com/store/cldpwr)** or a plain USB wall charger.",
                "If your computer has no microSD slot, add a **microSD to USB adapter**.",
            ],
            flag: Some("Never plug your Coldcard into a computer. Everything here is done offline (air-gapped)."),
            why: Some((
                "Why hardware, why offline",
                "On a phone or computer your keys share space with the internet. A hardware wallet generates and stores them on a dedicated offline device, so they are never exposed even if your everyday machine is compromised.",
            )),
            needs: &["Coldcard", "Seedplate + punch", "Casino dice"],
            backup_cta: false,
            device: NO_DEVICE,
        },
        // 2 · Inspect and power on
        Step {
            title: "Inspect and power on the Coldcard",
            goal: "Confirm the device reached you untouched before you trust it with keys.",
            actions: &[
                "Check the tamper-evident bag is sealed and undamaged. Keep it, it carries a unique serial number.",
                "Inside you get the Coldcard, a serialized tear-off tab, and a wallet backup card. The number on the tab should match the bag.",
                "Power the Coldcard from **Coldpower** or a USB wall charger, never from a computer.",
                "When it boots, confirm the serial number on screen matches the bag, then press the **checkmark**.",
            ],
            flag: Some("Some battery packs cut power to low-draw devices. A USB wall charger or Coldpower is more reliable."),
            why: Some((
                "Why the tamper bag matters",
                "A signing device is worth attacking before it ever reaches you. The sealed bag and matching serial numbers are your check that nobody opened or swapped the device in transit.",
            )),
            needs: &[],
            backup_cta: false,
            device: NO_DEVICE,
        },
        // 3 · Update the firmware
        Step {
            title: "Update the firmware",
            goal: "Get the device onto the latest signed firmware before creating any keys.",
            actions: &[
                "Download the latest firmware from the **[Coldcard upgrade page](https://coldcard.com/docs/upgrade/)**.",
                "**[Verify the download](https://coldcard.com/docs/upgrade/#dont-trust-verify-the-firmware)** before you use it.",
                "Copy the firmware file onto a microSD card and insert it into the Coldcard.",
                "Install it via **Advanced -> Upgrade Firmware -> From MicroSD**, then wait for the update to finish.",
            ],
            flag: None,
            why: None,
            needs: &["A microSD card"],
            backup_cta: false,
            device: NO_DEVICE,
        },
        // 4 · Set a PIN
        Step {
            title: "Set a strong PIN",
            goal: "Lock the device with a PIN only you know.",
            actions: &[
                "Select **Choose PIN Code**, then enter a prefix of at least 4 digits and write it on the backup card.",
                "Note the **two anti-phishing words** the Coldcard shows next. They appear every time you enter your prefix, and prove the device has not been tampered with or swapped.",
                "Enter a suffix of 4 to 6 digits and write that down too.",
                "Re-enter the prefix and suffix when asked, and check the anti-phishing words match what you wrote.",
            ],
            flag: Some("There is no way to recover this PIN. Keep it somewhere safe."),
            why: None,
            needs: &[],
            backup_cta: false,
            device: NO_DEVICE,
        },
        // 5 · Create seed with dice
        Step {
            title: "Create your seed with dice",
            goal: "Generate a 24-word key with your own added randomness.",
            actions: &[
                "From the main menu choose **New Wallet**, then press **4** to add dice rolls.",
                "Roll a real die at least **100 times**, entering each result. Do not fake it, this is your entropy.",
                "Write down the **24 words** in order on the backup card, then pass the Coldcard's confirmation quiz.",
                "With the first microSD card inserted, save an **encrypted backup** of the wallet to it.",
            ],
            flag: None,
            why: Some((
                "Why roll dice?",
                "So you do not have to fully trust the device's random number generator. Mixing in physical dice rolls means the final key is random even if the hardware's randomness were ever flawed.",
            )),
            needs: &["Backup card", "A pen"],
            backup_cta: true,
            device: NO_DEVICE,
        },
        // 6 · Verify by wipe & restore
        Step {
            title: "Verify by wipe and restore",
            goal: "Prove your written backup actually works, before funding it.",
            actions: &[
                "Record the wallet's **fingerprint** from **Advanced -> View Identity**.",
                "Wipe the seed: **Advanced -> Danger Zone -> Seed Functions -> Destroy Seed**, then read and accept the warnings.",
                "Re-enter your PIN, then go to **Import Existing -> 24 Words** and type your words back in. For the last word the Coldcard offers only the valid options; if yours is not listed, something earlier is wrong.",
                "Go back to **Advanced -> View Identity** and confirm the **fingerprint matches** the one you recorded.",
            ],
            flag: Some("If the fingerprint does not match, your words are wrong. Fix them before putting any bitcoin on this wallet."),
            why: Some((
                "Why wipe a working device",
                "An untested backup is not a backup. Destroying the seed and restoring it from your own handwriting is the only way to know those words really do bring the wallet back, and the fingerprint is what proves it.",
            )),
            needs: &[],
            backup_cta: false,
            device: NO_DEVICE,
        },
        // 7 · Add a passphrase
        Step {
            title: "Add a passphrase",
            goal: "Add an extra secret on top of your 24 words, which opens a separate and stronger wallet.",
            actions: &[
                "Select **Passphrase** from the main menu and read the warnings.",
                "Enter a phrase of at least 12 characters, mixing letters, numbers and symbols (up to 100 characters).",
                "Write it down and store it **apart from your seed words**.",
                "Press **APPLY**, then record the new **fingerprint** it produces. This is how you confirm you entered the passphrase correctly.",
                "With the second microSD card inserted, save an **encrypted backup of the passphrase** to it.",
            ],
            flag: Some("The Coldcard never stores your passphrase, you enter it every time you power on. It is as important as your 24 words, and the wallet backup you saved earlier does not contain it."),
            why: Some((
                "What a passphrase does",
                "It is combined with your 24 words to derive an entirely separate wallet. People sometimes call it a 25th word, but that is misleading: it is not a word from the seed list, it is any phrase you choose. Your 24 words alone open one wallet, and those same words plus the passphrase open a different one. Someone who found your written words could not reach your bitcoin without it.",
            )),
            needs: &[],
            backup_cta: false,
            device: NO_DEVICE,
        },
        // 8 · Back up in steel
        Step {
            title: "Back up in steel",
            goal: "Make your seed survive fire, water, and time.",
            actions: &[
                "Put the Seedplate on a solid surface. It is two-sided and holds 12 words per side.",
                "Punch the **first four letters** of each word, in order (column 1 is word 1). Four letters is enough, wallet software completes the rest.",
                "For example, for the word **certain** punch **C E R T** in column 1.",
                "Double-check every word. Steel is permanent, you cannot undo a punch.",
            ],
            flag: None,
            why: Some((
                "Why steel",
                "Paper burns and rots. A steel backup keeps your seed recoverable after a house fire or flood. Store it separately from your passphrase.",
            )),
            needs: &["Seedplate", "Center punch"],
            backup_cta: false,
            device: Device {
                frame: Frame::Desktop,
                shots: &[Shot {
                    image: "/img/seedplate.jpeg",
                    alt: "A Coinkite Seedplate with the first four letters of each word punched in",
                    caption: "A finished Seedplate, four letters punched per word",
                    img_w: 2000,
                    img_h: 1143,
                    pins: &[],
                }],
            },
        },
        // 9 · Backup checklist
        Step {
            title: "Check your backups",
            goal: "Confirm every piece is written down and stored before you put bitcoin on this wallet.",
            actions: &[
                "Your **24 words** are on the backup card and punched into steel.",
                "An **encrypted wallet backup** is on one microSD card, and an **encrypted passphrase backup** on the other.",
                "Your **passphrase** is written down and stored somewhere separate from the seed words.",
                "Both **fingerprints** are recorded: the seed on its own, and the seed with the passphrase applied.",
                "Your **PIN prefix, suffix and anti-phishing words** are on the backup card.",
            ],
            flag: Some("Never store your seed words together with your passphrase, and never put either online. No photos, no cloud, no password manager. This whole setup depends on staying offline."),
            why: Some((
                "How many copies should I keep?",
                "More steel plates in more locations protect you against fire and flood, but every extra copy is one more thing someone could find. Two locations you control is a reasonable balance. You can keep the passphrase alongside the fingerprint, just never alongside the seed words.",
            )),
            needs: &[],
            backup_cta: true,
            device: NO_DEVICE,
        },
        // 10 · Connect to Sparrow (desktop screenshot)
        Step {
            title: "Connect to Sparrow",
            goal: "Watch and spend from your Coldcard on desktop, with the keys staying offline.",
            actions: &[
                "Install **[Sparrow](https://sparrowwallet.com/download/)** on your computer (see the **[basic desktop guide](/guides/basic/desktop)** if you need it). You do not need to create a wallet in it.",
                "On the Coldcard, enter your passphrase, then export the wallet file to a microSD. If you only have two cards, use the passphrase card, since you have to enter the passphrase anyway.",
                "In Sparrow, import that file and follow **[Sparrow's Coldcard guide](https://sparrowwallet.com/docs/coldcard-wallet.html)**, which also covers receiving and sending.",
            ],
            flag: Some("Always enter your passphrase on the Coldcard before exporting a wallet file or signing a transaction."),
            why: None,
            needs: &["Sparrow (desktop)"],
            backup_cta: false,
            device: Device {
                frame: Frame::Desktop,
                shots: &[Shot {
                    image: "/guide-images/sparrow/sparrow_coldcard_import.png",
                    alt: "Sparrow, importing the Coldcard wallet",
                    caption: "Sparrow, importing the Coldcard",
                    img_w: 1026,
                    img_h: 771,
                    pins: &[],
                }],
            },
        },
    ],
    completion: Completion {
        title: "Your keys are on hardware",
        lede: "Your keys were generated offline, protected by a passphrase, backed up in steel, and Sparrow can now watch and spend without them ever touching the internet. One part left.",
        next_tier: Some(("Continue to part 2: run your own node", "/guides/intermediate/desktop/node")),
        backup_cta: false,
    },
};

// =============================================================================
// INTERMEDIATE PART 2 — run your own node. Content from the v1 node_setup markdown
// (node_faq1..5). Deliberately points at each project's own docs rather than
// re-documenting three separate builds.
// =============================================================================

pub static INTERMEDIATE_NODE_GUIDE: GuideV2 = GuideV2 {
    eyebrow: "Intermediate · Node",
    intro: Intro {
        title: "Run your own node",
        lede: "Stop trusting someone else's server. You will pick a node implementation, get it synced, and point Sparrow at it, so every block and every balance is checked by a machine you control.",
        chips: &["3 steps", "an evening, plus sync time", "part 2 of intermediate"],
        outcomes: &[
            "Your own full node validating every block",
            "Sparrow asking your node, not a stranger's server",
            "Better privacy, since nobody else sees your addresses",
        ],
        backup_cta: false,
    },
    steps: &[
        // 1 · Choose an implementation
        Step {
            title: "Choose your node",
            goal: "Pick the implementation that matches how much tinkering you want to do.",
            actions: &[
                "**Start9** is a full personal home server, GUI first and open source, with a marketplace of self-hosted apps. It is not bitcoin-only. **[Buy one](https://store.start9.com)**, **[build it](https://docs.start9.com/)**, or read the **[FAQ](https://start9.com/faq/)**.",
                "**MyNode** is bitcoin and Lightning only, and the friendliest of the three. Prebuilt units include a year of premium support. **[Buy one](https://www.mynodebtc.com/order_now)**, **[DIY](https://mynodebtc.github.io/)**, or read the **[docs](https://mynodebtc.github.io/intro/introduction.html)**.",
                "**RaspiBlitz** is the original DIY tinkerer's node, bitcoin and Lightning, with the advanced features behind SSH. **[Buy one](https://shop.fulmo.org/)** or **[follow the docs](https://docs.raspiblitz.org/docs/intro/)**.",
                "Any of the three will do the job. If you are unsure, pick the one whose documentation reads best to you.",
            ],
            flag: None,
            why: Some((
                "Why run a node?",
                "Your node downloads and checks every block and transaction itself, so you rely on no third party for what is true on the network. It also improves your privacy: Sparrow asks your node about your addresses instead of handing them to someone else's server.",
            )),
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Desktop,
                shots: &[
                    Shot {
                        image: "/guide-images/start9/start9_home.png",
                        alt: "The Start9 server dashboard",
                        caption: "Start9: a full personal home server, GUI first",
                        img_w: 3446,
                        img_h: 1988,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/mynode/mynode_ui.png",
                        alt: "The MyNode web interface",
                        caption: "MyNode: bitcoin and Lightning, the friendliest of the three",
                        img_w: 1688,
                        img_h: 1494,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/raspiblitz/raspiblitz_graphics.jpg",
                        alt: "A RaspiBlitz node with its display",
                        caption: "RaspiBlitz: the original DIY tinkerer's node",
                        img_w: 830,
                        img_h: 357,
                        pins: &[],
                    },
                ],
            },
        },
        // 2 · Build or buy, then sync
        Step {
            title: "Set it up and let it sync",
            goal: "Get the node running on your own network and fully caught up with the chain.",
            actions: &[
                "Buy the prebuilt unit or gather the parts, then follow that project's own setup guide start to finish.",
                "Install the **Bitcoin Core** service, and its **Electrum server** (electrs) service, which is what Sparrow will talk to.",
                "Let the initial sync finish. It downloads and verifies the entire chain, which takes anywhere from a few hours to a weekend.",
                "Leave the node powered on and connected. It should be running whenever you want to use your wallet.",
            ],
            flag: None,
            why: Some((
                "A word on hardware",
                "If you build your own, I recommend a used mini PC such as a Lenovo ThinkCentre or a Dell OptiPlex over a Raspberry Pi. For a little more money you get more memory, more storage and a faster processor, and the whole thing will feel less fragile. A Pi is still a fine way to start if you just want to get running cheaply.",
            )),
            needs: &[],
            backup_cta: false,
            device: NO_DEVICE,
        },
        // 3 · Point Sparrow at it
        Step {
            title: "Point Sparrow at your node",
            goal: "Move Sparrow off the public server and onto your own.",
            actions: &[
                "Find your node's **Electrum server address** in its dashboard (your node's app will show the host and port).",
                "In Sparrow open **File -> Preferences -> Server**, choose **Private Electrum**, and enter that address.",
                "Click **Test Connection**. Once it succeeds, apply and reopen your wallet.",
                "The status bar at the bottom of Sparrow should now show it is connected to your own server. See **[Sparrow's server docs](https://sparrowwallet.com/docs/connect-node.html)** if it will not connect.",
            ],
            flag: None,
            why: Some((
                "What changed",
                "Before this, a public server knew every address in your wallet and could link them together. Now that query never leaves your home, and the balances Sparrow shows you were verified by your own copy of the chain.",
            )),
            needs: &[],
            backup_cta: false,
            device: NO_DEVICE,
        },
    ],
    completion: Completion {
        title: "You have leveled up",
        lede: "Your keys live on dedicated hardware, backed up in steel, and your own node keeps the network honest for you. This is real self-custody.",
        next_tier: Some(("Level up to Advanced", "/guides/advanced/desktop")),
        backup_cta: false,
    },
};

// =============================================================================
// ADVANCED (level guide) — 2-of-3 multisig with three Coldcards, coordinated in
// Sparrow. Content from the v1 advanced_desktop_setup markdown (advanced_faq1..5),
// split into three parts: build it, use it, then optional hardening. Screenshots in
// assets/guide-images/multisig/ and coldcard/ are landscape => Frame::Desktop.
// FIRST PASS: menu paths come from the v1 markdown and need a walkthrough to verify.
// =============================================================================

pub static ADVANCED_MULTISIG_GUIDE: GuideV2 = GuideV2 {
    eyebrow: "Advanced · Multisig",
    intro: Intro {
        title: "Build a 2-of-3 multisig",
        lede: "No single device can lose or leak your bitcoin. You will set up three Coldcards, combine them into a 2-of-3 multisig with the air-gapped tool, and coordinate the whole thing from Sparrow. Two of the three keys sign any spend, so one lost or stolen device is survivable.",
        chips: &["8 steps", "a weekend", "part 1 of advanced"],
        outcomes: &[
            "A 2-of-3 multisig with no single point of failure",
            "Three signing devices you can store apart",
            "Your wallet output descriptor backed up safely",
        ],
        backup_cta: true,
    },
    steps: &[
        // 1 · Prerequisites
        Step {
            title: "Get up to speed",
            goal: "Have the pieces from the earlier tiers in place before you start.",
            actions: &[
                "Install and verify **Sparrow** if you have not already (the **[basic desktop guide](/guides/basic/desktop)** covers it).",
                "Set up your own node by following **[part 2 of the intermediate guide](/guides/intermediate/desktop/node)**.",
                "Read through **[part 1 of the intermediate guide](/guides/intermediate/desktop/hardware)** again, because you are about to do that Coldcard setup three times.",
                "If you can, use a **dedicated computer** whose only job is running bitcoin software.",
            ],
            flag: Some("Your own node is not strictly required, but without it a third party sees every address in your multisig. At this level that is worth avoiding."),
            why: Some((
                "Why a dedicated computer",
                "Sparrow never holds your keys, so a compromised computer cannot sign for you. It can still lie to you about addresses and balances. A machine that does nothing else has far less surface to attack.",
            )),
            needs: &[],
            backup_cta: false,
            device: NO_DEVICE,
        },
        // 2 · Decide the quorum
        Step {
            title: "Decide your quorum",
            goal: "Choose how many keys exist and how many are needed to spend.",
            actions: &[
                "Pick **N**, the number of signing devices, and **M**, how many must sign. This guide uses **2-of-3**.",
                "Choose your hardware. I use the latest **Coldcard**, but you can mix vendors to remove single-vendor risk, as long as every device supports multisig. More options at **[The Bitcoin Hole](https://thebitcoinhole.com/hardware-wallets)**.",
                "Count your cards: **two microSD cards per Coldcard** (one for the encrypted wallet backup, one for the passphrase), plus **one more** for the multisig setup itself.",
                "Decide now where each device and each backup will live. Different rooms is a start, different buildings is better.",
            ],
            flag: None,
            why: Some((
                "What M-of-N really buys you",
                "A 2-of-3 means any two keys can spend, and any one key can be lost or stolen without losing your bitcoin. That is the point: it removes the single point of failure a normal wallet has. It also adds complexity, so resist the urge to go bigger than you need.",
            )),
            needs: &[],
            backup_cta: false,
            device: NO_DEVICE,
        },
        // 3 · Plan the backups
        Step {
            title: "Plan your backups first",
            goal: "Know exactly what you will write down and where it goes, before any keys exist.",
            actions: &[
                "For **each** device you will record: the **seed words**, the **passphrase**, and the **master fingerprint**. Seed words go on their own paper, apart from the passphrase.",
                "On each Coldcard you will save an **encrypted seed backup** to one microSD and the **passphrase** to another. Use **[industrial grade cards](https://store.coinkite.com/store/microsd-cc)**.",
                "Punch each device's seed into its own **steel plate**, and store the plates in different locations.",
                "At the end, Sparrow gives you a **wallet output descriptor** as a PDF. You need it to rebuild the wallet, so keep it safe (it holds no private keys).",
            ],
            flag: Some("To recover a 2-of-3 you need two private keys AND the descriptor (the xpubs of all three). Losing every copy of the descriptor can strand your bitcoin even with the seeds in hand."),
            why: Some((
                "Do not invent your own scheme",
                "Every homemade backup trick (splitting words in half, personal ciphers, clever hiding places) has killed someone's coins. Stick to the standard pieces: seed words, passphrase, fingerprint, descriptor. Threat-model your storage, not your format.",
            )),
            needs: &[],
            backup_cta: true,
            device: Device {
                frame: Frame::Desktop,
                shots: &[Shot {
                    image: "/guide-images/multisig/wehodlbtc_xpub_backup.png",
                    alt: "An exported multisig text backup listing each xpub and fingerprint",
                    caption: "The exported xpubs and fingerprints, needed to rebuild the wallet",
                    img_w: 1311,
                    img_h: 221,
                    pins: &[],
                }],
            },
        },
        // 4 · Set up each Coldcard
        Step {
            title: "Set up each Coldcard",
            goal: "Create three independent single-sig wallets, one per device.",
            actions: &[
                "Run the full Coldcard setup from **[part 1 of the intermediate guide](/guides/intermediate/desktop/hardware)** on each device: inspect, update firmware, set a PIN, roll dice for the seed, verify by wipe and restore, add a passphrase.",
                "Record each device's **seed words, passphrase and master fingerprint** as you go. Keep them clearly labelled per device.",
                "Save each device's encrypted seed backup and passphrase to its own microSD cards.",
            ],
            flag: Some("The Coldcard does not remember your passphrase. Before you do anything multisig related, load it via Passphrase -> Restore Saved (or type it), and confirm the fingerprint matches that device."),
            why: None,
            needs: &["3 Coldcards", "Seedplates + punch"],
            backup_cta: false,
            device: NO_DEVICE,
        },
        // 5 · Export the xpubs
        Step {
            title: "Export each device's XPUB",
            goal: "Collect one ccxp file per Coldcard onto a single microSD card.",
            actions: &[
                "Load the passphrase on the first Coldcard, then insert the empty microSD card reserved for the multisig setup.",
                "Go to **Settings -> Multisig Wallets -> Export XPUB**. The device writes a **ccxp** file to the card.",
                "Repeat on the next Coldcard, onto the **same** card. Order does not matter.",
                "Stop after the second device: leave the third for the next step, which reads all the ccxp files at once.",
            ],
            flag: None,
            why: Some((
                "What is in a ccxp file",
                "It holds the device's extended public key, its master fingerprint and the derivation path. That is everything needed to build the multisig and watch it, and nothing that can spend. Public data, but it does reveal your addresses, so do not publish it.",
            )),
            needs: &["A spare microSD card"],
            backup_cta: false,
            device: NO_DEVICE,
        },
        // 6 · Create the air-gapped multisig
        Step {
            title: "Create the air-gapped multisig",
            goal: "Combine the three keys into one multisig wallet, entirely offline.",
            actions: &[
                "On the last Coldcard, load its passphrase and insert the microSD card holding the other ccxp files.",
                "Go to **Settings -> Multisig Wallets -> Create Airgapped**, read the screen, and press **OK**.",
                "Set **M**, the number of signers required, with the **7** and **9** keys. **N** is simply how many ccxp files it found, plus this device.",
                "Press **OK**, check the wallet summary, and confirm. The device writes two files: a **Coldcard multisig config** for the other devices, and an **Electrum skeleton** for Sparrow.",
            ],
            flag: None,
            why: None,
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Desktop,
                shots: &[
                    Shot {
                        image: "/guide-images/coldcard/coldcard_air_gapped.png",
                        alt: "The Coldcard air-gapped multisig creation screen",
                        caption: "Create Airgapped reads the ccxp files from the card",
                        img_w: 616,
                        img_h: 346,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/coldcard/coldcard_m_of_n.png",
                        alt: "The Coldcard screen for choosing the M of N threshold",
                        caption: "Set M with the 7 and 9 keys",
                        img_w: 361,
                        img_h: 208,
                        pins: &[],
                    },
                ],
            },
        },
        // 7 · Import the config to the others
        Step {
            title: "Teach the other Coldcards about the wallet",
            goal: "Give every device the multisig config so any of them can sign.",
            actions: &[
                "Eject the microSD card and insert it into one of the other Coldcards.",
                "Load that device's **passphrase first**, then go to **Settings -> Multisig Wallets -> Import from file** and pick the config.",
                "Repeat on the remaining device.",
            ],
            flag: Some("Import the config without loading the passphrase and the device builds the multisig from the wrong key, so it will not match. Load the passphrase every time, and check the fingerprint."),
            why: None,
            needs: &[],
            backup_cta: false,
            device: NO_DEVICE,
        },
        // 8 · Add it to Sparrow
        Step {
            title: "Add the wallet to Sparrow",
            goal: "Get a watch-and-spend view of the multisig on your desktop.",
            actions: &[
                "In Sparrow choose **File -> New Wallet**, name it, and click **Create Wallet**.",
                "Set **Policy Type** to **Multi Signature**, move the slider to your **M-of-N**, and leave **Script Type** on **Native SegWit (P2WSH)**.",
                "Insert the microSD card with the ccxp files. For **Keystore 1**, click **Air-Gapped Hardware Wallet**, find **Coldcard Multisig**, and **Import File**. Label it so you know which physical device it is.",
                "Repeat for each remaining keystore, then click **Apply**. You can set a Sparrow password to stop anyone at your computer from opening the wallet.",
                "When prompted, **Save PDF** of the wallet output descriptor and store it somewhere safe. Then click **OK** to finish.",
            ],
            flag: None,
            why: Some((
                "Why Sparrow holds no keys",
                "Sparrow only ever sees the extended public keys, so it can build transactions and show balances but never sign. Signing happens on the Coldcards, offline. That is what makes this an air-gapped setup.",
            )),
            needs: &["Sparrow (desktop)"],
            backup_cta: false,
            device: Device {
                frame: Frame::Desktop,
                shots: &[
                    Shot {
                        image: "/guide-images/multisig/sparrow_wallet_multisig_new_wallet.png",
                        alt: "Sparrow, creating a new wallet for the multisig",
                        caption: "File, New Wallet, then name it",
                        img_w: 1028,
                        img_h: 777,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/multisig/sparrow_new_wallet_multisig.png",
                        alt: "Sparrow, policy type set to multi signature with an M of N slider",
                        caption: "Policy Type Multi Signature, then your M-of-N",
                        img_w: 1028,
                        img_h: 769,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/multisig/sparrow_multisig_import.png",
                        alt: "Sparrow, importing a Coldcard multisig ccxp file",
                        caption: "Import each ccxp file as a keystore",
                        img_w: 1030,
                        img_h: 761,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/multisig/sparrow_multisig_keystore.png",
                        alt: "Sparrow, one of three keystores populated",
                        caption: "One keystore filled in, two to go",
                        img_w: 1029,
                        img_h: 764,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/multisig/sparrow_multisig_ready_to_import.png",
                        alt: "Sparrow, all keystores imported and ready to apply",
                        caption: "All keystores in, then Apply",
                        img_w: 1032,
                        img_h: 764,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/multisig/sparrow_multisig_backup.png",
                        alt: "Sparrow, prompt to save the wallet output descriptor as a PDF",
                        caption: "Save the output descriptor PDF and keep it safe",
                        img_w: 1029,
                        img_h: 767,
                        pins: &[],
                    },
                ],
            },
        },
    ],
    completion: Completion {
        title: "Your multisig is live",
        lede: "Three keys exist, any two can spend, and no single device or location can lose your bitcoin. Next, learn to actually move funds through it.",
        next_tier: Some(("Continue to part 2: receive and spend", "/guides/advanced/desktop/spending")),
        backup_cta: false,
    },
};

// =============================================================================
// ADVANCED PART 2 — receiving and spending from the multisig (the PSBT round trip).
// =============================================================================

pub static ADVANCED_SPENDING_GUIDE: GuideV2 = GuideV2 {
    eyebrow: "Advanced · Receive and spend",
    intro: Intro {
        title: "Receive and spend from a multisig",
        lede: "Receiving is no harder than a normal wallet. Spending is, because a transaction has to travel to two devices and back on a microSD card. You will do that round trip once here so it is familiar before it matters.",
        chips: &["5 steps", "about an hour", "part 2 of advanced"],
        outcomes: &[
            "Bitcoin received into the multisig",
            "A PSBT signed by two separate devices",
            "A spend broadcast from your own node",
        ],
        backup_cta: false,
    },
    steps: &[
        // 1 · Receive
        Step {
            title: "Receive to your multisig",
            goal: "Get an address and confirm the funds land.",
            actions: &[
                "Open the wallet in Sparrow and click **Receive**.",
                "Add a **Label** so you remember where the funds came from, then **copy** the address.",
                "Send a small test amount first. Nothing about a fresh multisig should be trusted with size until you have spent from it once.",
                "The payment appears under **Transactions** once your node sees it. One confirmation means it is yours and protected by the quorum.",
            ],
            flag: None,
            why: Some((
                "How many confirmations?",
                "One confirmation means it is in a block and protected by your multisig. For larger amounts wait for up to 6, which is the usual settled mark.",
            )),
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Desktop,
                shots: &[
                    Shot {
                        image: "/guide-images/multisig/receiving_to_multisig.png",
                        alt: "Sparrow, a receive address for the multisig wallet",
                        caption: "Label it, then copy the address",
                        img_w: 1300,
                        img_h: 916,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/multisig/transaction_received.png",
                        alt: "Sparrow, the received transaction in the transactions tab",
                        caption: "The payment lands under Transactions",
                        img_w: 1380,
                        img_h: 895,
                        pins: &[],
                    },
                ],
            },
        },
        // 2 · Build the transaction
        Step {
            title: "Create the transaction",
            goal: "Build an unsigned transaction in Sparrow.",
            actions: &[
                "Click **Send**. Paste the destination into **Pay to** and add a **Label**.",
                "Enter the **Amount**, then set your **fee rate**. If you are not in a rush, set it low.",
                "Check the address once more, then click **Create Transaction**.",
            ],
            flag: Some("Sparrow cannot sign this. Nothing leaves your wallet until two devices approve it, so take your time here."),
            why: None,
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Desktop,
                shots: &[Shot {
                    image: "/guide-images/multisig/sending_multisig_transaction.png",
                    alt: "Sparrow, composing a spend from the multisig wallet",
                    caption: "Address, label, amount, fee rate, then Create Transaction",
                    img_w: 1378,
                    img_h: 899,
                    pins: &[],
                }],
            },
        },
        // 3 · Finalize to a PSBT
        Step {
            title: "Verify and save the PSBT",
            goal: "Turn the transaction into a file your Coldcards can sign.",
            actions: &[
                "Review the inputs and outputs on the left, and confirm the destination address is right.",
                "Click **Details** for the technical view. Under **Signatures** you should see your multisig wallet listed.",
                "Click **Finalize Transaction for Signing**, then **Save Transaction** to write a **.psbt** file to a microSD card.",
            ],
            flag: None,
            why: Some((
                "What a PSBT is",
                "A partially signed bitcoin transaction: the whole transaction plus however many signatures it has collected so far. It carries no secrets, which is why it can safely ride a microSD card between your computer and your offline devices.",
            )),
            needs: &["A microSD card"],
            backup_cta: false,
            device: Device {
                frame: Frame::Desktop,
                shots: &[
                    Shot {
                        image: "/guide-images/multisig/verify_the_transaction.png",
                        alt: "Sparrow, verifying the transaction before signing",
                        caption: "Check inputs, outputs and signatures, then finalize",
                        img_w: 1381,
                        img_h: 901,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/multisig/save_the_transaction.png",
                        alt: "Sparrow, saving the transaction as a psbt file",
                        caption: "Save Transaction writes the .psbt file",
                        img_w: 1384,
                        img_h: 890,
                        pins: &[],
                    },
                ],
            },
        },
        // 4 · Sign on M devices
        Step {
            title: "Sign with two devices",
            goal: "Collect the signatures your quorum requires, offline.",
            actions: &[
                "Insert the microSD card into the first Coldcard and enter your PIN.",
                "Load the **passphrase** (**Passphrase -> Restore Saved**, or type it), then confirm the **fingerprint** is the one you expect.",
                "Choose **Ready to Sign** and pick the PSBT file. Verify the **amount**, **destination address** and **fee** on the device screen.",
                "Press **OK** to sign. The device writes a new file ending in **-part.psbt**.",
                "Repeat on the second device, and be sure to select the **-part.psbt** file, not the original.",
            ],
            flag: Some("Verify the address on the Coldcard screen, not just in Sparrow. Checking it on the offline device is the entire point of signing offline."),
            why: None,
            needs: &[],
            backup_cta: false,
            device: NO_DEVICE,
        },
        // 5 · Broadcast
        Step {
            title: "Broadcast the transaction",
            goal: "Send the fully signed transaction to the network through your own node.",
            actions: &[
                "Put the microSD card back in your computer and click **Load Transaction** in Sparrow.",
                "Select the fully signed file, likely ending in **-part-2.psbt**.",
                "Both signatures appear and **Broadcast Transaction** becomes clickable. Click it.",
                "Sparrow hands the transaction to your node, which relays it to the network.",
            ],
            flag: None,
            why: None,
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Desktop,
                shots: &[
                    Shot {
                        image: "/guide-images/multisig/signed_ready_to_broadcast.png",
                        alt: "Sparrow, both signatures collected and ready to broadcast",
                        caption: "Two signatures collected, ready to broadcast",
                        img_w: 1032,
                        img_h: 775,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/multisig/transaction_sent.png",
                        alt: "Sparrow, the transaction broadcast to the network",
                        caption: "Sent",
                        img_w: 1034,
                        img_h: 765,
                        pins: &[],
                    },
                ],
            },
        },
    ],
    completion: Completion {
        title: "You spent from a multisig",
        lede: "You have moved bitcoin that required two independent devices to approve. That round trip is the skill worth having, so run it once more before you store anything serious here.",
        next_tier: Some(("Optional: harden it further", "/guides/advanced/desktop/hardening")),
        backup_cta: false,
    },
};

// =============================================================================
// ADVANCED PART 3 — optional hardening (duress wallets, SeedXOR, HSM). Reference
// material from advanced_faq5, deliberately gated behind an "optional" framing
// because every item here adds a way to lose funds.
// =============================================================================

pub static ADVANCED_HARDENING_GUIDE: GuideV2 = GuideV2 {
    eyebrow: "Advanced · Hardening",
    intro: Intro {
        title: "Harden it further",
        lede: "Optional extras for specific threat models. Every one of them adds complexity, and complexity is how people lose bitcoin. Read them, take what genuinely fits your situation, and skip the rest without guilt.",
        chips: &["3 topics", "optional", "part 3 of advanced"],
        outcomes: &[
            "A decoy wallet for a coerced-access scenario",
            "Seed backups split without weakening them",
            "A sense of which extras are worth the risk",
        ],
        backup_cta: false,
    },
    steps: &[
        // 1 · Duress / decoy
        Step {
            title: "Duress and decoy wallets",
            goal: "Have something to hand over if you are ever forced to open a device.",
            actions: &[
                "**The passphrase trick:** your real wallet only exists once the passphrase is loaded. Send a small amount to the Coldcard's plain seed-words wallet (no passphrase) and it becomes a believable decoy.",
                "**A duress PIN:** the Coldcard can hold a second PIN that opens a separate wallet. Fund it with an amount you are willing to lose. See **[the docs](https://coldcard.com/docs/settings/#duress-pin)**.",
                "Whichever you use, keep a little real activity in the decoy. An empty wallet is not convincing.",
            ],
            flag: Some("A decoy only helps if you can stay calm and consistent under pressure. Practise opening it, and never hint that anything else exists."),
            why: Some((
                "Why this exists",
                "Also called a $5 wrench attack. Once someone is physically threatening you, cryptography is irrelevant, so the goal shifts to having something plausible to give up. This is about physical safety more than key security.",
            )),
            needs: &[],
            backup_cta: false,
            device: NO_DEVICE,
        },
        // 2 · SeedXOR
        Step {
            title: "Split a seed with SeedXOR",
            goal: "Store a seed in multiple pieces without making it easier to attack.",
            actions: &[
                "Never split a seed by simply cutting the word list in half. It leaks most of your key and makes brute force far easier. **[Here is why](https://www.youtube.com/watch?v=p5nSibpfHYE)**.",
                "Use the Coldcard's **SeedXOR** instead: it splits a seed into parts where every part is a valid but useless seed on its own, and all parts are required to rebuild the real one.",
                "Store each part in a different location, and record which wallet the set belongs to.",
                "Read the **[SeedXOR documentation](https://seedxor.com/)** before you commit any funds to this.",
            ],
            flag: Some("Every extra part is another thing you can lose. If any single part goes missing, that seed is gone. Consider whether your multisig quorum already solves the problem you are reaching for here."),
            why: None,
            needs: &[],
            backup_cta: false,
            device: NO_DEVICE,
        },
        // 3 · HSM
        Step {
            title: "Signing without touching the device",
            goal: "Know what exists, in case you need automated or remote signing.",
            actions: &[
                "The Coldcard's **HSM mode** and **CKBunker** let it sign according to preset rules without someone pressing buttons.",
                "This suits treasuries and services, not personal savings. Read the **[official HSM docs](https://coldcard.com/docs/hsm/)** if it applies to you.",
                "For a normal setup, skip this. Pressing OK yourself is a feature, not a chore.",
            ],
            flag: None,
            why: None,
            needs: &[],
            backup_cta: false,
            device: NO_DEVICE,
        },
    ],
    completion: Completion {
        title: "That is the whole path",
        lede: "Keys on dedicated hardware, spread across a quorum, backed up in steel and verified by your own node. Keep it simple from here, test your recovery once a year, and enjoy actually owning your bitcoin.",
        next_tier: None,
        backup_cta: false,
    },
};

// =============================================================================
// BULL BITCOIN (Basic, mobile) — the spending wallet. Two wallets from one seed:
// Secure Bitcoin (on-chain) and Instant payments (Liquid + Lightning). Facts about
// the app's architecture come from the project README
// (github.com/SatoshiPortal/bullbitcoin-mobile). Screenshots in
// assets/guide-images/bull/ are 1080x2424 phone shots.
// NOTE: the app blocks screenshots on the recovery-words and confirm-words screens,
// so step 3 shows the dashboard warning before and after instead. The
// choose-your-backup-method screen is still missing from the asset set.
// =============================================================================

const BULL_W: u32 = 1080;
const BULL_H: u32 = 2424;

pub static BULL_GUIDE: GuideV2 = GuideV2 {
    eyebrow: "Basic · Bull Bitcoin",
    intro: Intro {
        title: "Set up Bull Bitcoin",
        lede: "A self-custodial wallet built for spending. You get two wallets from one backup: an on-chain Bitcoin wallet for savings, and an instant wallet on Liquid for everyday payments. Lightning is temporarily out of service, explained on the first step. Everything this guide covers works normally. Create the wallet, write down your recovery words, and learn to receive and send.",
        chips: &["6 steps", "about 20 min", "best for spending"],
        outcomes: &[
            "A self-custodied wallet, with the keys on your phone",
            "One set of recovery words written down safely",
            "The confidence to receive, spend and recover",
        ],
        backup_cta: true,
    },
    steps: &[
        // 1 · Install and open
        Step {
            title: "Install and open Bull",
            goal: "Get through the first-run screens with the settings you actually want.",
            actions: &[
                "Install Bull Bitcoin, open it, and tap **Next** on the welcome screen.",
                "Set your **theme, language and default currency**. You can change all of these later.",
                "Bull asks whether to send anonymised error logs. **Yes** or **No** both work; **No** is the more private choice.",
                "The last screen lists features to explore later. Tap **Get started**.",
            ],
            // Bull's Lightning rail runs on Boltz, which suspended service in August 2026.
            // Flagged here on step 1 because the Lightning tab is still visible in the app
            // and in this guide's screenshots. Remove once Bull ships a replacement.
            flag: Some("Lightning is temporarily unavailable in Bull. The swap provider it relied on suspended service, and Bull is working on a replacement. On-chain Bitcoin and Liquid both work normally, your funds are unaffected, and everything in this guide is on-chain. See **[Bull's announcement](https://www.bullbitcoin.com/blog/boltz-has-suspended-its-swap-services-your-funds-are-safe-here-is-what-it-means-for-bull-wallet-users)** for details."),
            why: Some((
                "What Bull actually is",
                "A self-custodial wallet with an optional exchange bolted on. The keys are generated on your phone and never leave it, and you do not need a Bull Bitcoin account to use any wallet feature. The buy and sell buttons only matter if you choose to use them.",
            )),
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Phone,
                shots: &[
                    Shot {
                        image: "/guide-images/bull/bull-onboarding-01-welcome.png",
                        alt: "Bull Bitcoin welcome screen, own your money",
                        caption: "Bull, welcome",
                        img_w: BULL_W,
                        img_h: BULL_H,
                        pins: &[Pin { n: 1, x: 9.0, y: 92.0, label: "Tap Next" }],
                    },
                    Shot {
                        image: "/guide-images/bull/bull-onboarding-02-customize.png",
                        alt: "Bull, customize theme, language and default currency",
                        caption: "Bull, customise your experience",
                        img_w: BULL_W,
                        img_h: BULL_H,
                        pins: &[Pin { n: 2, x: 9.0, y: 34.0, label: "Set theme, language and currency" }],
                    },
                    Shot {
                        image: "/guide-images/bull/bull-onboarding-03-telemetry.png",
                        alt: "Bull, asking to opt in to anonymised error reporting",
                        caption: "Bull, error reporting is optional",
                        img_w: BULL_W,
                        img_h: BULL_H,
                        pins: &[Pin { n: 3, x: 72.0, y: 92.0, label: "Choose Yes or No" }],
                    },
                    Shot {
                        image: "/guide-images/bull/bull-onboarding-04-features.png",
                        alt: "Bull, a list of features to try, with Get started",
                        caption: "Bull, then Get started",
                        img_w: BULL_W,
                        img_h: BULL_H,
                        pins: &[Pin { n: 4, x: 9.0, y: 92.0, label: "Tap Get started" }],
                    },
                ],
            },
        },
        // 2 · Create the wallet
        Step {
            title: "Create your wallet",
            goal: "Generate your keys on the phone, which creates both of Bull's wallets at once.",
            actions: &[
                "Tap **Create New Wallet**. Your keys are generated on the device.",
                "**Advanced Options** is optional. It lets you route through **Tor** or point the app at your own **Electrum** and **Mempool** servers, and you can set all of it later in App Settings.",
                "Bull opens on your dashboard with two wallets: **Secure Bitcoin** on the Bitcoin network, and **Instant payments** on Liquid.",
                "You'll notice the banner **Autoswap is active**, click on it for details. Left on, it sweeps the instant wallet back down to a target balance once it goes over a maximum, moving the surplus into Secure Bitcoin so spending money does not quietly pile up on Liquid. Default is on, feel free to toggle it off if you don't plan to use liquid.",
            ],
            flag: None,
            why: Some((
                "Why two wallets, and what Liquid is",
                "Both wallets come from the same recovery words, so one backup covers both. Secure Bitcoin holds real on-chain bitcoin. Instant payments sits on Liquid, a sidechain that settles in seconds for a fraction of a cent, which is what makes small payments practical. Liquid is not the Bitcoin main chain though: it is run by a federation of functionaries, so you are trusting that group in a way you are not on-chain. Treat the instant wallet as pocket money and keep savings in Secure Bitcoin. Autoswap is Bull's way of enforcing exactly that split for you: set a target and a maximum, and anything above the maximum is swept into Secure Bitcoin, with a fee ceiling so it will not move funds on an expensive day. It is a swap under the hood, so it is one of the things that stopped working when Boltz suspended service, and you can turn it off outright.",
            )),
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Phone,
                shots: &[
                    Shot {
                        image: "/guide-images/bull/bull-onboarding-05-create-or-recover.png",
                        alt: "Bull, create new wallet or recover wallet",
                        caption: "Bull, create a new wallet",
                        img_w: BULL_W,
                        img_h: BULL_H,
                        pins: &[Pin { n: 1, x: 9.0, y: 78.0, label: "Tap Create New Wallet" }],
                    },
                    Shot {
                        image: "/guide-images/bull/bull-onboarding-06-advanced-options.png",
                        alt: "Bull, advanced options with Tor and custom servers",
                        caption: "Bull, advanced options (optional)",
                        img_w: BULL_W,
                        img_h: BULL_H,
                        pins: &[Pin { n: 2, x: 9.0, y: 22.0, label: "Optional: Tor and your own servers" }],
                    },
                    Shot {
                        image: "/guide-images/bull/bull-backup-01-warning.png",
                        alt: "Bull dashboard showing the Secure Bitcoin and Instant payments wallets",
                        caption: "Two wallets, one seed",
                        img_w: BULL_W,
                        img_h: BULL_H,
                        pins: &[Pin { n: 3, x: 4.0, y: 52.0, label: "Secure Bitcoin and Instant payments" }],
                    },
                    Shot {
                        image: "/guide-images/bull/bull-autoswap-settings.png",
                        alt: "Bull Auto Transfer Settings with target and maximum instant wallet balances, a maximum transfer fee and Secure Bitcoin as the recipient",
                        caption: "Autoswap: sweep the surplus into Secure Bitcoin",
                        img_w: BULL_W,
                        img_h: BULL_H,
                        pins: &[Pin { n: 4, x: 4.0, y: 35.0, label: "Target and maximum balances" }],
                    },
                ],
            },
        },
        // 3 · Back up
        Step {
            title: "Write down your recovery words",
            goal: "Save your seed words, they are the only way to recover your wallet.",
            actions: &[
                "On the dashboard, tap **Protect your bitcoin. Back up your wallet now.**",
                "Choose the **physical backup** option, which shows you the words to write down. (The encrypted vault option stores an encrypted copy with a provider; this guide uses paper.)",
                "Write the words down **in order** on [paper](/downloads/seed-backup-sheet.html), then confirm them when Bull asks.",
                "Once confirmed, the warning disappears from your dashboard. That is how you know the backup is done.",
            ],
            flag: Some("Bull blocks screenshots on the recovery-words screens, which is a good thing: never photograph or type your words into anything. Paper only, stored somewhere only you can reach."),
            why: Some((
                "One backup, both wallets",
                "The same words restore Secure Bitcoin and Instant payments, so there is only ever one set to protect. Bull keeps nagging you on the dashboard until you have proven the backup by typing the words back in, which is worth doing properly. If the app is ever wiped or broken by an update, those words are the whole recovery path.",
            )),
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Phone,
                shots: &[
                    Shot {
                        image: "/guide-images/bull/bull-backup-01-warning.png",
                        alt: "Bull dashboard showing the back up your wallet warning",
                        caption: "Tap the backup warning on the dashboard",
                        img_w: BULL_W,
                        img_h: BULL_H,
                        pins: &[Pin { n: 1, x: 4.0, y: 38.0, label: "Tap the backup warning" }],
                    },
                    Shot {
                        image: "/guide-images/bull/bull-backup-02-done.png",
                        alt: "Bull dashboard with the backup warning gone",
                        caption: "Warning gone once the backup is confirmed",
                        img_w: BULL_W,
                        img_h: BULL_H,
                        pins: &[Pin { n: 4, x: 4.0, y: 30.0, label: "Warning gone, backup done" }],
                    },
                ],
            },
        },
        // 4 · Receive
        Step {
            title: "Receive bitcoin",
            goal: "Get an address and watch a payment arrive.",
            actions: &[
                "Tap **Receive**, then pick the network. Choose **Bitcoin** to follow this guide. The screen also offers **Liquid**, and a **Lightning** tab that is out of service for now.",
                "Share the **QR code** or copy the address. You can add an optional amount and a private note.",
                "Your balance updates when the payment arrives, and the wallet card shows it.",
                "Tap the transaction to see it while it is **Pending**.",
                "Once it is in a block, the status becomes **Confirmed** with a confirmation time.",
            ],
            flag: None,
            why: Some((
                "Which network should I receive on?",
                "On-chain Bitcoin for anything you intend to keep, because it settles on the main chain. Liquid for small everyday amounts, because a small on-chain payment can cost more in fees than it is worth. Bull nudges you toward the sensible one based on the amount, and warns you when a payment is uneconomical. Lightning would normally be the other everyday option, but it currently out of service. For now, stick with on-chain (Secure Bitcoin).",
            )),
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Phone,
                shots: &[
                    Shot {
                        image: "/guide-images/bull/bull-receive-01-address.png",
                        alt: "Bull receive screen with Bitcoin, Lightning and Liquid tabs",
                        caption: "Pick a network, then share the QR or address",
                        img_w: BULL_W,
                        img_h: BULL_H,
                        pins: &[
                            Pin { n: 1, x: 4.0, y: 16.0, label: "Choose Bitcoin" },
                            Pin { n: 2, x: 4.0, y: 68.0, label: "Copy the address" },
                        ],
                    },
                    Shot {
                        image: "/guide-images/bull/bull-receive-02-dashboard-funded.png",
                        alt: "Bull dashboard showing a received balance",
                        caption: "The balance updates",
                        img_w: BULL_W,
                        img_h: BULL_H,
                        pins: &[Pin { n: 3, x: 4.0, y: 56.0, label: "Your balance, now funded" }],
                    },
                    Shot {
                        image: "/guide-images/bull/bull-receive-03-wallet-detail.png",
                        alt: "Bull secure bitcoin wallet detail with manage coins and a pending transaction",
                        caption: "Inside Secure Bitcoin: coin control and history",
                        img_w: BULL_W,
                        img_h: BULL_H,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/bull/bull-receive-04-pending-details.png",
                        alt: "Bull transaction details showing a pending receive",
                        caption: "Pending, waiting for a block",
                        img_w: BULL_W,
                        img_h: BULL_H,
                        pins: &[Pin { n: 4, x: 4.0, y: 49.0, label: "Status: Pending" }],
                    },
                    Shot {
                        image: "/guide-images/bull/bull-receive-05-confirmed-details.png",
                        alt: "Bull transaction details showing a confirmed receive",
                        caption: "Confirmed, with the time it landed",
                        img_w: BULL_W,
                        img_h: BULL_H,
                        pins: &[Pin { n: 5, x: 4.0, y: 49.0, label: "Status: Confirmed" }],
                    },
                ],
            },
        },
        // 5 · Send
        Step {
            title: "Send bitcoin",
            goal: "Send a payment and choose what you pay in fees.",
            actions: &[
                "Open your **Secure Bitcoin** wallet and tap **Send**. Scan the recipient's code with **Open the Camera**, or paste their address into **Recipient's address**, then tap **Continue**.",
                "Enter the **amount**. Tap the arrow beside it to switch between sats and another currency, add a **note** to remember what the payment was for, or flip **MAX** to send the whole balance. Tap **Continue**.",
                "The confirm screen lists the wallet it leaves **From**, the address it goes **To**, your note, the amount and the **network fee**. Confirm the address here.",
                "To change what you pay, tap **Fee Priority**: **Fastest**, **Economic**, **Slow**, or a custom rate. Bull shows each one in sats per vByte, in sats, and in your currency.",
                "Tap **Confirm**. Bull signs and broadcasts it, then shows **Successfully Sent**.",
                "Tap **View Details** for the transaction ID and status. While it says **Pending** you can **Accelerate** it, which re-sends the same payment with a higher fee.",
                "Back in the wallet the payment sits under **Pending** with your note, and the balance already accounts for it.",
            ],
            flag: Some("Always re-read the address before you confirm. Bitcoin transactions cannot be reversed."),
            why: Some((
                "Choosing a fee",
                "Fees pay the miners who include your transaction in a block, and they rise and fall with demand. Slow is fine when nothing is waiting on it; Fastest is worth it when someone is standing in front of you. If you pick too low and get impatient, Accelerate re-sends it with a higher fee.",
            )),
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Phone,
                shots: &[
                    Shot {
                        image: "/guide-images/bull/bull-send-01-recipient.png",
                        alt: "Bull send screen offering a QR scan or a pasted recipient address",
                        caption: "Scan the code, or paste the address",
                        img_w: BULL_W,
                        img_h: BULL_H,
                        pins: &[Pin { n: 1, x: 4.0, y: 81.0, label: "Paste the address here" }],
                    },
                    Shot {
                        image: "/guide-images/bull/bull-send-02-amount.png",
                        alt: "Bull send screen with the amount, an optional note and a MAX toggle",
                        caption: "Amount, note, and MAX for the lot",
                        img_w: BULL_W,
                        img_h: BULL_H,
                        pins: &[Pin { n: 2, x: 4.0, y: 24.0, label: "Enter the amount" }],
                    },
                    Shot {
                        image: "/guide-images/bull/bull-send-03-confirm.png",
                        alt: "Bull confirm send screen showing from, to, note, amount, network fee and fee priority",
                        caption: "Check everything, then Confirm",
                        img_w: BULL_W,
                        img_h: BULL_H,
                        pins: &[
                            Pin { n: 3, x: 4.0, y: 42.0, label: "Read the address" },
                            Pin { n: 4, x: 4.0, y: 62.0, label: "Tap to change the fee" },
                            Pin { n: 5, x: 4.0, y: 78.0, label: "Confirm to broadcast" },
                        ],
                    },
                    Shot {
                        image: "/guide-images/bull/bull-send-04-fee.png",
                        alt: "Bull select network fee with fastest, economic, slow and a custom rate",
                        caption: "Fastest, Economic, Slow, or your own rate",
                        img_w: BULL_W,
                        img_h: BULL_H,
                        pins: &[Pin { n: 4, x: 4.0, y: 28.0, label: "Pick a fee" }],
                    },
                    Shot {
                        image: "/guide-images/bull/bull-send-05-sent.png",
                        alt: "Bull confirmation that the payment was successfully sent",
                        caption: "Sent",
                        img_w: BULL_W,
                        img_h: BULL_H,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/bull/bull-send-06-pending-details.png",
                        alt: "Bull transaction details with the transaction ID, a pending status and Accelerate",
                        caption: "Pending, with Accelerate if you need it",
                        img_w: BULL_W,
                        img_h: BULL_H,
                        pins: &[Pin { n: 6, x: 4.0, y: 72.0, label: "Status: Pending" }],
                    },
                    Shot {
                        image: "/guide-images/bull/bull-send-07-wallet-pending.png",
                        alt: "Bull Secure Bitcoin wallet with the pending send at the top and a reduced balance",
                        caption: "Pending in your history, balance updated",
                        img_w: BULL_W,
                        img_h: BULL_H,
                        pins: &[Pin { n: 7, x: 4.0, y: 48.0, label: "Your pending payment" }],
                    },
                ],
            },
        },
        // 6 · Recover
        Step {
            title: "If you lose your phone",
            goal: "Get both wallets back on a new device using your written words.",
            actions: &[
                "Install Bull on the new phone and tap **Recover Wallet** instead of creating one.",
                "Enter your recovery words **in order**.",
                "Both **Secure Bitcoin** and **Instant payments** come back, since they share the one seed. Give it a moment to sync.",
                "You can also restore into any other **BIP39** wallet, meaning any wallet that recovers from recovery words, though only Bull will rebuild the Liquid side.",
            ],
            flag: Some("Treat the lost phone as compromised. After recovering, create a brand new wallet (a fresh set of keys with its own new recovery words) and move all funds to it. Anyone who ends up with the old phone or its written words could otherwise take your bitcoin."),
            why: Some((
                "Why this step matters more here",
                "This is a hot wallet on a phone, and phones break, get stolen, and occasionally get bricked by a bad app update. That is survivable when the amounts are small and the words are on paper, and unrecoverable when they are not. Practise a recovery once with a trivial amount so you know it works before you rely on it.",
            )),
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Phone,
                shots: &[Shot {
                    image: "/guide-images/bull/bull-onboarding-05-create-or-recover.png",
                    alt: "Bull, recover wallet option on the first screen",
                    caption: "Bull, recover an existing wallet",
                    img_w: BULL_W,
                    img_h: BULL_H,
                    pins: &[Pin { n: 1, x: 9.0, y: 86.0, label: "Tap Recover Wallet" }],
                }],
            },
        },
    ],
    completion: Completion {
        title: "You are self-custodied",
        lede: "Your keys are on your phone and your words are on paper. Keep the amounts here to what you would carry in a wallet, and when your stack grows, move the savings onto dedicated hardware.",
        next_tier: Some(("Level up to Intermediate", "/guides/intermediate/desktop")),
        backup_cta: false,
    },
};

// =============================================================================
// NUNCHUK (Basic, mobile) — the wallet you grow into. Deliberately kept to a
// SINGLE-SIG hot wallet at this tier: Nunchuk's multisig and collaborative custody
// belong to the Advanced tier, and this guide only points forward to them.
// Facts verified against github.com/nunchuk-io/nunchuk-android.
// PRIVACY NOTE: Nunchuk does NOT block screenshots on its seed-phrase screens, so
// the three seed-bearing shots here are IRREVERSIBLY REDACTED (mosaiced) copies. Never
// replace them with the raw captures.
//
// PIN CONVENTION for "tap Continue": place the pin at the LEFT END of the Continue
// button, vertically centred on it, so it lands in the same spot on every screen.
// Measured on these 1080x2424 shots: a full-width Continue is 996x126 at +42+2193
// => x 6.0, y 93.1. The modal Continue is 764x126 at +158+1168 => x 17.0, y 50.8.
// =============================================================================

const NUN_W: u32 = 1080;
const NUN_H: u32 = 2424;

pub static NUNCHUK_GUIDE: GuideV2 = GuideV2 {
    eyebrow: "Basic · Nunchuk",
    intro: Intro {
        title: "Set up Nunchuk",
        lede: "Start with one key on your phone, in an app that can grow all the way to multisig later. You will create a single-signature hot wallet, write down your recovery words, and learn to receive and send, without changing apps when you level up.",
        chips: &["6 steps", "about 20 min", "grows with you"],
        outcomes: &[
            "A single-sig wallet, with the key on your phone",
            "Your 24 recovery words written down safely",
            "A clear path to hardware signers and multisig",
        ],
        backup_cta: true,
    },
    steps: &[
        // 1 · Install, skip the account
        Step {
            title: "Install and continue as guest",
            goal: "Open Nunchuk without handing over an email address.",
            actions: &[
                "Install Nunchuk and open it. The first screen offers sign-in options.",
                "Tap **Continue as guest**. You do not need an account for a self-custodial wallet.",
                "Nunchuk's Home screen explains the two ways in: add a key first, or create a hot wallet straight away.",
            ],
            flag: None,
            why: Some((
                "Why skip the account",
                "An account unlocks Nunchuk's paid, assisted services, and none of that is needed here. Continuing as a guest keeps your keys and your wallet entirely local, with no email tying your bitcoin to your identity. You can always sign in later if you decide you want those services.",
            )),
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Phone,
                shots: &[
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-01-signin.png",
                        alt: "Nunchuk sign-in screen with a continue as guest option",
                        caption: "Nunchuk, continue as guest",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        pins: &[Pin { n: 2, x: 4.0, y: 62.0, label: "Tap Continue as guest" }],
                    },
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-02-home-welcome.png",
                        alt: "Nunchuk home screen with add key, create hot wallet and recover options",
                        caption: "Nunchuk, the Home screen",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        pins: &[Pin { n: 3, x: 4.0, y: 71.0, label: "Create hot wallet" }],
                    },
                ],
            },
        },
        // 2 · Create the hot wallet
        Step {
            title: "Create a hot wallet",
            goal: "Generate one key on the phone and get a single-sig wallet from it.",
            actions: &[
                "Tap **Create hot wallet**.",
                "Read the **hot wallet** explainer. It says plainly that a hot wallet is connected to the internet, and that large amounts belong on cold storage or multisig. That is the honest framing, and it is why this tier is for spending money.",
                "Tap **Continue**. Nunchuk creates **My hot wallet**, marked **Single-sig**.",
            ],
            flag: None,
            why: Some((
                "Why start single-sig here",
                "One key is one thing to back up and one thing to understand. Nunchuk can do 2-of-3 multisig with hardware signers, but that is the Advanced tier and it deserves its own guide. Learn the single-key basics in this app first, then the upgrade path never involves changing wallets.",
            )),
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Phone,
                shots: &[
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-02-home-welcome.png",
                        alt: "Nunchuk home screen with the create hot wallet option",
                        caption: "Tap Create hot wallet",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        pins: &[Pin { n: 1, x: 4.0, y: 71.0, label: "Create hot wallet" }],
                    },
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-03-hot-wallet-explainer.png",
                        alt: "Nunchuk hot wallet explainer screen",
                        caption: "Nunchuk is honest about what a hot wallet is",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        pins: &[Pin { n: 2, x: 4.0, y: 86.0, label: "Read it, then Continue" }],
                    },
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-04-wallets-list.png",
                        alt: "Nunchuk wallets list showing My hot wallet marked single-sig",
                        caption: "My hot wallet, marked Single-sig",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        pins: &[Pin { n: 3, x: 4.0, y: 20.0, label: "Your new wallet" }],
                    },
                ],
            },
        },
        // 3 · Back up
        Step {
            title: "Write down your recovery words",
            goal: "Save the 24 words that are your only way back into this wallet.",
            actions: &[
                "Open the wallet. A banner reads **Please write down the seed phrase**. Tap **Do it now**.",
                "Nunchuk warns that **this action cannot be repeated**. Have paper and a pen ready, then tap **Continue**.",
                "Write all **24 words** down in order on [paper](/downloads/seed-backup-sheet.html). Check the spelling, then tap **Continue**.",
                "Confirm the words Nunchuk asks for (it picks three at random), then tap **Continue**.",
                "You land on your **Key Info** screen. Rename the key if you like, and note that **View seed phrase** lives here if you ever need it again. Tap the **back arrow** in the top left when you are done.",
                "The warning banner disappears and the wallet header turns from amber to navy. That is how you know it is done.",
            ],
            flag: Some("Unlike some wallets, Nunchuk does not block screenshots on this screen, so it is entirely up to you: never photograph your words and never type them into anything. Paper only. The two screenshots here are deliberately blurred for exactly that reason."),
            why: Some((
                "One shot at this",
                "Nunchuk shows the phrase once during setup and says so up front. You can still reach it later via Keys, then your key, then View seed phrase, but treat the first showing as the real one. Write it down properly rather than planning to come back.",
            )),
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Phone,
                shots: &[
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-05-backup-warning.png",
                        alt: "Nunchuk wallet with a write down the seed phrase banner",
                        caption: "Tap Do it now on the banner",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        pins: &[Pin { n: 1, x: 4.0, y: 13.0, label: "Tap Do it now" }],
                    },
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-06-cannot-repeat-dialog.png",
                        alt: "Nunchuk dialog warning that this action cannot be repeated",
                        caption: "This action cannot be repeated",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        pins: &[Pin { n: 2, x: 17.0, y: 50.8, label: "Tap Continue" }],
                    },
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-07-seed-phrase-redacted.png",
                        alt: "Nunchuk seed phrase screen, words deliberately obscured",
                        caption: "Your 24 words (hidden here on purpose)",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        pins: &[Pin { n: 3, x: 6.0, y: 93.1, label: "Write them all down, then Continue" }],
                    },
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-08-confirm-seed-redacted.png",
                        alt: "Nunchuk confirm seed phrase screen, candidate words obscured",
                        caption: "Confirm the words it asks for",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        pins: &[Pin { n: 4, x: 6.0, y: 93.1, label: "Pick the right words, then Continue" }],
                    },
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-09-key-info.png",
                        alt: "Nunchuk key info screen after verifying the seed phrase",
                        caption: "Name your key, then tap the back arrow",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        pins: &[Pin { n: 5, x: 13.0, y: 9.5, label: "Back arrow, top left" }],
                    },
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-10-backup-done.png",
                        alt: "Nunchuk wallet with the backup warning gone",
                        caption: "Banner gone, header now navy",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        pins: &[],
                    },
                ],
            },
        },
        // 4 · Receive
        Step {
            title: "Receive bitcoin",
            goal: "Share an address and watch the payment arrive.",
            actions: &[
                "In the wallet, tap **Receive**. An empty wallet also shows its address straight away.",
                "**Copy address** or **Share address** and give it to the sender.",
                "The payment appears as **Pending confirmations** and your balance updates.",
                "Tap it for the details, where **View on blockchain explorer** lets you check it yourself.",
                "Once it is included in a block the badge becomes a **confirmation count**, climbing with every block that follows.",
            ],
            flag: None,
            why: Some((
                "When is it really yours?",
                "Pending confirmations means the payment has been broadcast but is not in a block yet. One confirmation is enough for small amounts; for larger ones wait for up to 6. View on blockchain explorer lets you confirm all of it independently, rather than taking the app's word for it.",
            )),
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Phone,
                shots: &[
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-10-backup-done.png",
                        alt: "Nunchuk wallet with a receive address and QR code on the empty wallet screen",
                        caption: "Tap Receive, or use the address shown here",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        pins: &[
                            Pin { n: 1, x: 45.0, y: 27.5, label: "Tap Receive" },
                            Pin { n: 2, x: 4.0, y: 76.0, label: "Copy or share the address" },
                        ],
                    },
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-11-received.png",
                        alt: "Nunchuk wallet showing a received pending payment",
                        caption: "Received, pending confirmations",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        // x=45 sits in the empty gap between the truncated address (ends
                        // ~25%) and the status badge (starts ~64%). Anything past ~60%
                        // covers the badge text the pin is pointing at.
                        pins: &[Pin { n: 3, x: 45.0, y: 37.0, label: "Pending confirmations, balance updated" }],
                    },
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-12-receive-details.png",
                        alt: "Nunchuk transaction details for a received payment",
                        caption: "Details, with a link to a block explorer",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        pins: &[Pin { n: 4, x: 42.0, y: 17.0, label: "Tap through for the details" }],
                    },
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-19-wallet-funded.png",
                        alt: "Nunchuk wallet showing confirmed receives with a confirmation count",
                        caption: "Confirmed, with the count climbing",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        // confirmation pills are 260x48 at +778+870 and +778+1050
                        // Matches pin 3's x: same screen layout, same carousel.
                        pins: &[Pin { n: 5, x: 45.0, y: 36.9, label: "Confirmation count" }],
                    },
                ],
            },
        },
        // 5 · Send
        Step {
            title: "Send bitcoin",
            goal: "Send a payment out of the wallet.",
            actions: &[
                "Open your wallet and tap **Send**.",
                "Enter the **amount**, or tap **Send all** to sweep the wallet (**Switch to USD** if you prefer fiat), then tap **Continue**.",
                "Paste or scan the recipient's **address**. Nunchuk breaks it into blocks so you can check it a chunk at a time. Add a **note** if you want to remember what it was for.",
                "Tap **Create transaction** to accept the suggested fee, or **Customize transaction** to control it yourself.",
                "Customising lets you **subtract the fee from the amount**, choose which **coins** to spend, and tick **Manual fee rate** to set your own sat/vB against the current priority, standard and economical rates.",
                "Check the address, fee and total on **Confirm transaction**, then tap **Confirm and create transaction**.",
                "The transaction now exists but is **unsigned**, marked **Pending signatures**. Tap **Sign** next to your key.",
                "It flips to **Ready to broadcast** with **enough signatures collected**. Tap **Broadcast transaction** to send it.",
                "The send appears in your wallet as **Pending confirmations**, with your note beneath it.",
            ],
            flag: Some("Always re-read the recipient address before you confirm. Bitcoin transactions cannot be reversed."),
            why: Some((
                "Why sending takes two steps here",
                "Most wallets send in one tap. Nunchuk splits it into create, then sign, because it is built for multisig: there a transaction is drafted first and then signed by several keys, often held by different people or on different devices. With a single key you simply do both yourself, so it is one extra tap. Nothing is wrong, and it is worth getting used to: this is the exact same flow you will use unchanged if you ever move up to a 2-of-3.",
            )),
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Phone,
                shots: &[
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-19-wallet-funded.png",
                        alt: "Nunchuk funded wallet with the Send button",
                        caption: "Tap Send",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        // Send/Receive/View coins are 126px circles at x 169/464/772, y 611.
                        pins: &[Pin { n: 1, x: 17.5, y: 27.8, label: "Tap Send" }],
                    },
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-20-send-amount.png",
                        alt: "Nunchuk new transaction amount entry with send all",
                        caption: "Enter the amount, then Continue",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        pins: &[Pin { n: 2, x: 6.0, y: 93.1, label: "Tap Continue" }],
                    },
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-21-send-address.png",
                        alt: "Nunchuk new transaction with the recipient address and a note",
                        caption: "Address and note, then create or customise",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        // Create transaction 996x126 at +42+2025 => left end x 6.0, centre y 86.1
                        pins: &[
                            Pin { n: 3, x: 4.0, y: 17.0, label: "Paste or scan the address" },
                            Pin { n: 4, x: 6.0, y: 86.1, label: "Create transaction" },
                        ],
                    },
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-22-customize-fee.png",
                        alt: "Nunchuk customize transaction with fee settings and coin selection",
                        caption: "Optional: fee settings and coin selection",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        pins: &[Pin { n: 5, x: 4.0, y: 36.0, label: "Fee and coin options" }],
                    },
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-23-manual-fee-rate.png",
                        alt: "Nunchuk manual fee rate in sats per vbyte with the current rates",
                        caption: "Manual fee rate, with the current rates shown",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-24-confirm-transaction.png",
                        alt: "Nunchuk confirm transaction screen with address, fee, total and input coins",
                        caption: "Check it, then confirm and create",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        pins: &[Pin { n: 6, x: 6.0, y: 93.1, label: "Confirm and create transaction" }],
                    },
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-25-pending-signatures.png",
                        alt: "Nunchuk transaction pending signatures with a sign button beside the key",
                        caption: "Created but unsigned: tap Sign",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        // Sign button 184x95 at +854+1100 => centre y 47.3, pin just left of it
                        pins: &[Pin { n: 7, x: 74.0, y: 47.3, label: "Tap Sign" }],
                    },
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-26-ready-to-broadcast.png",
                        alt: "Nunchuk transaction signed and ready to broadcast",
                        caption: "Signed, ready to broadcast",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        pins: &[Pin { n: 8, x: 6.0, y: 93.0, label: "Broadcast transaction" }],
                    },
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-27-send-pending.png",
                        alt: "Nunchuk wallet showing the send pending with a note",
                        caption: "Sent, pending confirmations",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        // Send-to row's Pending pill is 353x48 at +685+870
                        pins: &[Pin { n: 9, x: 60.0, y: 36.9, label: "Your send, pending" }],
                    },
                ],
            },
        },
        // 6 · Recover / grow
        Step {
            title: "If you lose your phone",
            goal: "Know how to restore this wallet onto a new device.",
            actions: &[
                "Install Nunchuk on the new phone and choose **Continue as guest**.",
                "Tap **Recover existing wallet**, at the bottom of the wallet-type list.",
                "Choose **Recover hot wallet**. The other entries are for hardware devices, group wallets and descriptors.",
                "Type your **24 words separated by a space**, in order, all into the one box. Nunchuk suggests each word as you type, and **Continue** stays greyed out until the whole phrase is valid.",
                "Your wallet comes back with its **full history**, confirmations and all. Your words are standard **BIP39**, so any wallet that restores from recovery words can recover this key too.",
            ],
            flag: Some("Treat the lost phone as compromised. After recovering, create a brand new wallet (a fresh set of keys with its own new recovery words) and move all funds to it. Anyone who ends up with the old phone or its written words could otherwise take your bitcoin."),
            why: None,
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Phone,
                shots: &[
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-02-home-welcome.png",
                        alt: "Nunchuk home screen after choosing continue as guest",
                        caption: "Continue as guest on the new phone",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        pins: &[Pin { n: 1, x: 4.0, y: 78.0, label: "Continue as guest" }],
                    },
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-18-wallet-types.png",
                        alt: "Nunchuk wallet type list with recover existing wallet at the bottom",
                        caption: "Recover existing wallet, at the bottom",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        pins: &[Pin { n: 2, x: 4.0, y: 79.0, label: "Recover existing wallet" }],
                    },
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-15-recover-methods.png",
                        alt: "Nunchuk list of recovery methods including recover hot wallet",
                        caption: "Choose Recover hot wallet",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        pins: &[Pin { n: 3, x: 4.0, y: 73.8, label: "Recover hot wallet" }],
                    },
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-16-recover-enter-words.png",
                        alt: "Nunchuk recover hot wallet screen for entering the seed phrase",
                        caption: "All 24 words in one box, separated by spaces",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        pins: &[Pin { n: 4, x: 4.0, y: 21.0, label: "Type the words separated by spaces" }],
                    },
                    Shot {
                        image: "/guide-images/nunchuk/nunchuk-17-recovered.png",
                        alt: "Nunchuk wallet restored with its full transaction history and confirmations",
                        caption: "Restored, history and confirmations intact",
                        img_w: NUN_W,
                        img_h: NUN_H,
                        pins: &[Pin { n: 5, x: 38.0, y: 55.0, label: "History back, confirmations intact" }],
                    },
                ],
            },
        },
    ],
    completion: Completion {
        title: "You are self-custodied",
        lede: "One key, on your phone, with the words on paper. When you are ready for a hardware signer or a 2-of-3 multisig, you already have the app for it.",
        next_tier: Some(("Level up to Intermediate", "/guides/intermediate/desktop")),
        backup_cta: false,
    },
};

#[cfg(test)]
mod content_tests {
    use super::*;

    /// Every guide reachable from a wallet picker or a level part.
    fn all_guides() -> Vec<(&'static str, &'static GuideV2)> {
        let mut v: Vec<(&'static str, &'static GuideV2)> =
            ["cove", "bull", "nunchuk", "sparrow"]
                .iter()
                .filter_map(|id| find_guide_v2(id).map(|g| (*id, g)))
                .collect();
        for part in INTERMEDIATE_PARTS.iter().chain(ADVANCED_PARTS.iter()) {
            v.push((part.id, part.guide));
        }
        v
    }

    fn has_markdown(s: &str) -> bool {
        s.contains("**") || (s.contains("](") && s.contains('['))
    }

    /// `actions` and `flag` are rendered through `inline()`; nothing else is. Markdown
    /// left in any other field prints as literal `**[text](url)**` on the page.
    #[test]
    fn markdown_only_appears_in_inline_parsed_fields() {
        for (id, g) in all_guides() {
            assert!(
                !has_markdown(g.intro.lede),
                "{id}: intro.lede is not inline-parsed"
            );
            assert!(
                !has_markdown(g.completion.lede),
                "{id}: completion.lede is not inline-parsed"
            );
            for (i, s) in g.steps.iter().enumerate() {
                assert!(
                    !has_markdown(s.goal),
                    "{id} step {i}: goal is not inline-parsed"
                );
                if let Some((summary, body)) = s.why {
                    assert!(
                        !has_markdown(summary),
                        "{id} step {i}: why summary is not inline-parsed"
                    );
                    assert!(
                        !has_markdown(body),
                        "{id} step {i}: why body is not inline-parsed"
                    );
                }
                for n in s.needs {
                    assert!(
                        !has_markdown(n),
                        "{id} step {i}: needs pill is not inline-parsed"
                    );
                }
                for shot in s.device.shots {
                    assert!(
                        !has_markdown(shot.caption),
                        "{id} step {i}: caption is not inline-parsed"
                    );
                    assert!(
                        !has_markdown(shot.alt),
                        "{id} step {i}: alt text is not inline-parsed"
                    );
                    for p in shot.pins {
                        assert!(
                            !has_markdown(p.label),
                            "{id} step {i}: pin label is not inline-parsed"
                        );
                    }
                }
            }
        }
    }

    /// Pins reference the numbered actions beside them, so a pin number with no matching
    /// action is a dangling reference the reader cannot resolve.
    #[test]
    fn pin_numbers_reference_a_real_action() {
        for (id, g) in all_guides() {
            for (i, s) in g.steps.iter().enumerate() {
                let n_actions = s.actions.len();
                for shot in s.device.shots {
                    for p in shot.pins {
                        assert!(
                            (p.n as usize) <= n_actions && p.n >= 1,
                            "{id} step {i}: pin {} but the step has {n_actions} actions",
                            p.n
                        );
                    }
                }
            }
        }
    }

    /// Nothing reachable without passing the under-construction gate may mention
    /// COLDCARD. The Intermediate and Advanced guides are built around it and are
    /// gated; if a level is ever un-gated, or a Coldcard reference creeps into a Basic
    /// guide, this fails rather than quietly recommending a compromised device.
    #[test]
    fn ungated_guides_never_mention_coldcard() {
        let banned = ["coldcard", "coinkite", "seedplate", "mk3", "mk4"];
        for level in crate::guides::ALL_LEVELS {
            if level.under_construction {
                continue;
            }
            let mut texts: Vec<&str> =
                vec![level.intro, level.title, level.subtitle];
            // Part guides as well as wallet guides: Intermediate and Advanced carry no
            // wallets, so `level.wallets` alone would scan nothing for them.
            let mut guides: Vec<&'static GuideV2> = level
                .wallets
                .iter()
                .filter_map(|w| find_guide_v2(w))
                .collect();
            for part in parts_for_level(level.id) {
                texts.push(part.name);
                texts.push(part.tagline);
                texts.extend(part.highlights.iter().copied());
                guides.push(part.guide);
            }
            for g in guides {
                {
                    texts.push(g.intro.title);
                    texts.push(g.intro.lede);
                    texts.extend(g.intro.outcomes.iter().copied());
                    texts.push(g.completion.title);
                    texts.push(g.completion.lede);
                    for s in g.steps {
                        texts.push(s.title);
                        texts.push(s.goal);
                        texts.extend(s.actions.iter().copied());
                        texts.extend(s.needs.iter().copied());
                        if let Some(f) = s.flag {
                            texts.push(f);
                        }
                        if let Some((a, b)) = s.why {
                            texts.push(a);
                            texts.push(b);
                        }
                        for shot in s.device.shots {
                            texts.push(shot.alt);
                            texts.push(shot.caption);
                            for pin in shot.pins {
                                texts.push(pin.label);
                            }
                        }
                    }
                }
            }
            for t in texts {
                let lower = t.to_lowercase();
                for b in banned {
                    assert!(
                        !lower.contains(b),
                        "level '{}' is live and mentions '{}': {}",
                        level.id,
                        b,
                        t
                    );
                }
            }
        }
    }
}
