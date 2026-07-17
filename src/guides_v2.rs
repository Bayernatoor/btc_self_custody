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

/// Look up a v2 guide attached to a whole LEVEL (not a wallet), e.g. Intermediate,
/// which is one guide across its (single) platform. Some => the level page renders
/// StepperV2 directly instead of a wallet picker / step nav.
pub fn find_level_guide_v2(level_id: &str) -> Option<&'static GuideV2> {
    match level_id {
        "intermediate" => Some(&INTERMEDIATE_GUIDE),
        _ => None,
    }
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
                "The passphrase is a 13th secret that is not part of the 24 words. If someone finds your written words, they still cannot reach your bitcoin without the passphrase. Kept apart, neither piece is enough on its own.",
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
                        img_h: 588,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow-onboarding-4.png",
                        alt: "Sparrow connection intro, Later or Offline Mode button",
                        caption: "On the last screen, click Later or Offline Mode",
                        img_w: 598,
                        img_h: 579,
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
                "Click **Apply** to save the wallet, then set a wallet password (this encrypts the file on your computer and is not your passphrase) or click **No Password**.",
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
                        img_h: 812,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow-onboarding-15-first-wallet-walletpassword.png",
                        alt: "Sparrow, set a wallet password or No Password",
                        caption: "Set a wallet password, or click No Password",
                        img_w: 1074,
                        img_h: 813,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow-onboarding-17-first-wallet-final.png",
                        alt: "Sparrow, the wallet open and loading its history",
                        caption: "Your wallet, loading its history",
                        img_w: 1072,
                        img_h: 835,
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
                        image: "/guide-images/sparrow/sparrow_wallet_recovery_dropdown.png",
                        alt: "Sparrow, choose the number of words from the dropdown",
                        caption: "Choose your word count",
                        img_w: 1023,
                        img_h: 766,
                        pins: &[],
                    },
                    Shot {
                        image: "/guide-images/sparrow/sparrow_wallet_recovery_seed.png",
                        alt: "Sparrow, enter the words and passphrase, then discover wallet",
                        caption: "Enter your words and passphrase, then Discover Wallet",
                        img_w: 1031,
                        img_h: 775,
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
// one guide for all OSes. Content condensed from the v1 hardware_wallet_setup and
// node_setup markdown. Most steps have no screenshot (NO_DEVICE => single column);
// buy/docs links live inline in the actions via the [text](url) parser.
// =============================================================================

pub static INTERMEDIATE_GUIDE: GuideV2 = GuideV2 {
    eyebrow: "Intermediate · Desktop",
    intro: Intro {
        title: "Hardware wallet + your own node",
        lede: "Real self-custody starts here. You will generate your keys on a dedicated hardware device (never a phone), connect it to Sparrow on your desktop, back them up in steel, and run your own Bitcoin node.",
        chips: &["2 parts", "a few evenings", "for a serious stack"],
        outcomes: &[
            "Keys generated on a Coldcard, fully offline",
            "A steel backup that survives fire and water",
            "Your own node validating every block",
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
        // 2 · Inspect & update
        Step {
            title: "Inspect and update the Coldcard",
            goal: "Confirm the device is genuine and on the latest firmware.",
            actions: &[
                "Check the tamper-evident bag's serial number matches the one shown on the Coldcard when it powers on.",
                "Download the latest firmware and **[verify it](https://coldcard.com/docs/upgrade/)** before use.",
                "Copy the firmware to a microSD and install it via **Advanced -> Upgrade Firmware -> From MicroSD**.",
            ],
            flag: None,
            why: None,
            needs: &["A microSD card"],
            backup_cta: false,
            device: NO_DEVICE,
        },
        // 3 · Set a PIN
        Step {
            title: "Set a strong PIN",
            goal: "Lock the device with a PIN only you know.",
            actions: &[
                "Choose **Choose PIN Code**, then set a prefix and a suffix (4 to 6 digits each).",
                "Note the **two anti-phishing words** shown after the prefix; they prove the device has not been tampered with.",
                "Write the prefix, suffix, and anti-phishing words on the included backup card.",
            ],
            flag: Some("There is no way to recover this PIN. Keep it somewhere safe."),
            why: None,
            needs: &[],
            backup_cta: false,
            device: NO_DEVICE,
        },
        // 4 · Create seed with dice
        Step {
            title: "Create your seed with dice",
            goal: "Generate a 24-word key with your own added randomness.",
            actions: &[
                "From the main menu choose **New Wallet**, then press **4** to add dice rolls.",
                "Roll a real die at least **100 times**, entering each result. Do not fake it, this is your entropy.",
                "Write down the **24 words** in order, then pass the Coldcard's confirmation quiz.",
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
        // 5 · Verify by wipe & restore
        Step {
            title: "Verify by wipe and restore",
            goal: "Prove your written backup actually works, before funding it.",
            actions: &[
                "Record the wallet's **fingerprint** from **Advanced -> View Identity**.",
                "Wipe the seed: **Advanced -> Danger Zone -> Seed Functions -> Destroy Seed**.",
                "Re-import your 24 words, then confirm the **fingerprint matches** the one you recorded.",
            ],
            flag: Some("If the fingerprint does not match, your words are wrong. Fix them before putting any bitcoin on this wallet."),
            why: None,
            needs: &[],
            backup_cta: false,
            device: NO_DEVICE,
        },
        // 6 · Add a passphrase
        Step {
            title: "Add a passphrase",
            goal: "Add a secret 25th word that creates a separate, stronger wallet.",
            actions: &[
                "Choose **Passphrase**, read the warnings, and enter a phrase of at least 12 characters.",
                "Write it down and store it **apart from your seed words**, and record the new **fingerprint** it produces.",
                "Save an encrypted backup of the passphrase to the second microSD card.",
            ],
            flag: Some("Your passphrase is as important as your seed words, and the Coldcard never stores it, you enter it every time."),
            why: Some((
                "What a passphrase does",
                "It is combined with your 24 words to derive an entirely separate wallet. Even someone who found your 24 words could not reach your bitcoin without it.",
            )),
            needs: &[],
            backup_cta: false,
            device: NO_DEVICE,
        },
        // 7 · Back up in steel
        Step {
            title: "Back up in steel",
            goal: "Make your seed survive fire, water, and time.",
            actions: &[
                "On the Seedplate, punch the **first four letters** of each word, in order (column 1 is word 1).",
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
            device: NO_DEVICE,
        },
        // 8 · Connect to Sparrow (desktop screenshot)
        Step {
            title: "Connect to Sparrow",
            goal: "Watch and spend from your Coldcard on desktop, with the keys staying offline.",
            actions: &[
                "Install **[Sparrow](https://sparrowwallet.com/download/)** on your computer (see the **[basic desktop guide](/guides/basic/desktop)** if you need it).",
                "On the Coldcard, enter your passphrase, then export the wallet file to a microSD.",
                "In Sparrow, import that file and follow **[Sparrow's Coldcard guide](https://sparrowwallet.com/docs/coldcard-wallet.html)**.",
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
        // 9 · Run your own node (choice, links out)
        Step {
            title: "Run your own node",
            goal: "Validate every block yourself, without trusting anyone else's server.",
            actions: &[
                "**Start9** is a full personal home server, GUI-first: **[buy one](https://store.start9.com)** or **[build it](https://docs.start9.com/)**.",
                "**MyNode** is Bitcoin and Lightning, very beginner-friendly: **[buy one](https://www.mynodebtc.com/order_now)** or **[DIY](https://mynodebtc.github.io/)**.",
                "**RaspiBlitz** is the classic DIY tinkerer's node: **[buy one](https://shop.fulmo.org/)** or **[follow the docs](https://docs.raspiblitz.org/docs/intro/)**.",
                "Once it has synced, point Sparrow at your own node instead of a public server.",
            ],
            flag: None,
            why: Some((
                "Why run a node?",
                "Your node downloads and checks every block and transaction itself, so you rely on no third party for what is true on the network. It also improves your privacy, since Sparrow asks your node about your addresses instead of someone else's server.",
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
