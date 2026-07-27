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
        "blue" => Some(&BLUE_GUIDE),
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
pub fn find_level_part(level_id: &str, part_id: &str) -> Option<&'static LevelPart> {
    parts_for_level(level_id).iter().find(|p| p.id == part_id)
}

/// Sentinel for a step with no screenshot: the renderer shows a single centered
/// column (no device frame) when `image` is empty.
const NO_DEVICE: Device = Device { frame: Frame::Desktop, shots: &[] };

// =============================================================================
// COVE (Basic) — content adapted from the BlueWallet guide, rendered for Cove.
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
            backup_cta: true,
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
        backup_cta: true,
    },
};

// =============================================================================
// BLUE WALLET (Basic) — content adapted from the v1 BlueWallet markdown guide.
// Screenshots live in assets/guide-images/bluewallet/ (dims vary per image, set
// inline so each device frame matches its screenshot's aspect, no cropping).
// =============================================================================

pub static BLUE_GUIDE: GuideV2 = GuideV2 {
    eyebrow: "Basic · Blue Wallet",
    intro: Intro {
        title: "Set up Blue Wallet",
        lede: "A radically simple, self-custodied wallet for your spending stack. You will create a wallet, write down your recovery words, and learn to receive and send bitcoin.",
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
            goal: "Make a new Blue Wallet with the keys generated on your phone.",
            actions: &[
                "Open Blue Wallet and tap **Add now**.",
                "Give it a name, set **Type** to **Bitcoin**, then tap **Create**.",
                "Blue Wallet then shows your **12 recovery words** to back up next.",
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
                shots: &[Shot {
                    image: "/guide-images/bluewallet/bluewallet_wallet_creation.jpg",
                    alt: "Blue Wallet, name the wallet and pick the Bitcoin type",
                    caption: "Blue Wallet, create a wallet",
                    img_w: 629,
                    img_h: 1058,
                    pins: &[
                        Pin { n: 1, x: 50.0, y: 30.0, label: "Name your wallet" },
                        Pin { n: 2, x: 50.0, y: 55.0, label: "Set Type to Bitcoin" },
                        Pin { n: 3, x: 50.0, y: 85.0, label: "Tap Create" },
                    ],
                }],
            },
        },
        // 2 · Back up recovery words
        Step {
            title: "Write down your recovery words",
            goal: "Save the words that are the only way to recover your bitcoin if you lose your phone.",
            actions: &[
                "Write all **12 words** down **in order** on paper, exactly as shown.",
                "Double-check every word and its spelling against the screen.",
                "Tap **Ok, I wrote it down**, and confirm **Yes, I have** when asked.",
            ],
            flag: Some("Never take a photo of these words or type them into any app. Anyone who reads them can take your bitcoin. Paper only."),
            why: Some((
                "Why write them on paper",
                "Your recovery words are the wallet. Anything digital (a screenshot, a note, a cloud backup) can be reached by an attacker. A hand-written copy kept offline cannot.",
            )),
            needs: &[],
            backup_cta: true,
            device: Device {
                frame: Frame::Phone,
                shots: &[Shot {
                    image: "/guide-images/bluewallet/bluewallet_backup_confirmation.png",
                    alt: "Blue Wallet, confirm you have written down your recovery words",
                    caption: "Blue Wallet, confirm your backup (words-display shot pending)",
                    img_w: 628,
                    img_h: 1235,
                    pins: &[Pin { n: 1, x: 50.0, y: 62.0, label: "Confirm you wrote the words down" }],
                }],
            },
        },
        // 3 · Receive
        Step {
            title: "Receive bitcoin",
            goal: "Get an address so someone can send bitcoin to your wallet.",
            actions: &[
                "Open your wallet and tap **Receive**.",
                "Blue Wallet shows a **QR code** and an address starting with **bc1**.",
                "Let the sender scan the QR, or tap the address to copy it, or tap **Share**.",
            ],
            flag: None,
            why: Some((
                "When is it really mine?",
                "A payment must be included in a block (confirmed). It first shows as pending at 0 confirmations. One confirmation takes about 10 minutes on average, and six is the usual settled mark.",
            )),
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Phone,
                shots: &[Shot {
                    image: "/guide-images/bluewallet/bluewallet_receive_address.png",
                    alt: "Blue Wallet, receive address and QR code",
                    caption: "Blue Wallet, receive screen",
                    img_w: 607,
                    img_h: 1230,
                    pins: &[
                        Pin { n: 1, x: 50.0, y: 40.0, label: "Your address as a QR code" },
                        Pin { n: 2, x: 50.0, y: 64.0, label: "The bc1 address" },
                        Pin { n: 3, x: 50.0, y: 90.0, label: "Share it" },
                    ],
                }],
            },
        },
        // 4 · Send
        Step {
            title: "Send bitcoin",
            goal: "Send bitcoin from your wallet to someone else's bitcoin address.",
            actions: &[
                "Open your wallet and tap **Send**.",
                "Enter the **amount** and paste the recipient's **address**.",
                "Set a **fee**, tap **Next**, review carefully, then tap **Send now**.",
            ],
            flag: Some("Always re-read the address before sending. Bitcoin transactions cannot be reversed."),
            why: Some((
                "A note on fees",
                "Block space is limited, so fees rise with demand. Blue Wallet suggests a fee, but if you are in no rush you can set it lower to save sats.",
            )),
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Phone,
                shots: &[Shot {
                    image: "/guide-images/bluewallet/bluewallet_sending_page.png",
                    alt: "Blue Wallet, send compose screen",
                    caption: "Blue Wallet, send screen",
                    img_w: 632,
                    img_h: 1245,
                    pins: &[
                        Pin { n: 1, x: 50.0, y: 30.0, label: "Enter the amount" },
                        Pin { n: 2, x: 50.0, y: 46.0, label: "Paste the recipient address" },
                        Pin { n: 3, x: 50.0, y: 80.0, label: "Set the fee, then Next" },
                    ],
                }],
            },
        },
        // 5 · Recover
        Step {
            title: "If you lose your phone",
            goal: "Know how to recover your bitcoin onto a new device using your written words.",
            actions: &[
                "Install Blue Wallet (or any **BIP39** wallet) on a new phone.",
                "Tap **Add now**, then **Import wallet**.",
                "Enter your **words in order**, separated by spaces, then tap **Import**.",
            ],
            flag: Some("Treat the lost phone as compromised. After recovering, create a brand new wallet (a fresh set of keys with its own new recovery words, not the same phrase again) and move all funds to it. Anyone who ends up with the old phone or its written words could otherwise take your bitcoin."),
            why: None,
            needs: &[],
            backup_cta: false,
            device: Device {
                frame: Frame::Phone,
                shots: &[Shot {
                    image: "/guide-images/bluewallet/bluewallet_import_wallet.png",
                    alt: "Blue Wallet, import an existing wallet",
                    caption: "Blue Wallet, import / recover",
                    img_w: 614,
                    img_h: 1235,
                    pins: &[Pin { n: 1, x: 50.0, y: 72.0, label: "Enter your words, then Import" }],
                }],
            },
        },
    ],
    completion: Completion {
        title: "You are self-custodied",
        lede: "Your bitcoin is in your hands now. Keep your recovery words safe, and when your stack grows, level up.",
        next_tier: Some(("Level up to Intermediate", "/guides/intermediate/desktop")),
        backup_cta: true,
    },
};

// =============================================================================
// SPARROW (Basic, desktop) — single-sig + BIP39 passphrase, 24 words. Screenshots
// in assets/guide-images/sparrow/ are landscape, so every step with shots uses
// Frame::Desktop => the renderer STACKS the actions above a full-width window frame
// (see .g2-stack) and shows a per-shot caption. The wallet-creation shots
// (sparrow-onboarding-*.png) already have red arrows drawn on them pointing at the
// control to click, so those shots use no pins. Step 1 (download/verify) has no shot.
// =============================================================================

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
            backup_cta: true,
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
        backup_cta: true,
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
        backup_cta: true,
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
        backup_cta: true,
    },
};
