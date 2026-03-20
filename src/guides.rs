//! Guide definitions: all wallet, level, and platform metadata lives here.
//!
//! Single source of truth — no guide content is hardcoded in route components.

/// A downloadable wallet application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletDef {
    pub id: &'static str,
    pub name: &'static str,
    pub tagline: &'static str,
    pub description: &'static str,
    pub color: &'static str,
    pub logo: &'static str,
    pub logo_alt: &'static str,
    /// FAQ directory name under src/faqs/ (used by Stepper)
    pub faq_dir: &'static str,
    pub downloads: &'static [DownloadLink],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadLink {
    pub label: &'static str,
    pub url: &'static str,
    pub logo: &'static str,
    pub logo_alt: &'static str,
    /// Which platforms this download applies to
    pub platforms: &'static [&'static str],
}

/// A product to purchase (hardware wallet, seedplate, etc.)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductLink {
    pub name: &'static str,
    pub description: &'static str,
    pub url: &'static str,
    pub logo: &'static str,
    pub logo_alt: &'static str,
    pub logo_width: &'static str,
    pub logo_height: &'static str,
}

/// A guide level (Basic, Intermediate, Advanced).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuideLevelDef {
    pub id: &'static str,
    pub name: &'static str,
    pub subtitle: &'static str,
    pub title: &'static str,
    pub quote: &'static str,
    pub quote_author: &'static str,
    pub intro: &'static str,
    pub platforms: &'static [&'static str],
    pub wallets: &'static [&'static str],
    /// FAQ directory for this level's stepper (if no per-wallet FAQ)
    pub faq_dir: Option<&'static str>,
    /// Products to purchase (hardware, etc.)
    pub products: &'static [ProductLink],
    /// For multi-step levels (intermediate), sub-steps
    pub steps: &'static [GuideStep],
}

/// A sub-step within a guide level (e.g., "Hardware Wallet Setup", "Node Setup")
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuideStep {
    pub id: &'static str,
    pub name: &'static str,
    pub title: &'static str,
    pub icon: &'static str,
    pub icon_alt: &'static str,
    pub faq_dir: &'static str,
    pub products: &'static [ProductLink],
    pub next_step: Option<&'static str>,
    pub next_step_label: Option<&'static str>,
    pub next_step_button_label: Option<&'static str>,
}

// =============================================================================
// WALLET DEFINITIONS
// =============================================================================

pub static BLUE_WALLET: WalletDef = WalletDef {
    id: "blue",
    name: "Blue Wallet",
    tagline: "Radically Simple, Extremely Powerful.",
    description: "A freedom and self-sovereign tool, disguised as a cute little Blue app in your pocket.",
    color: "#1a578f",
    logo: "/bluewallet_logo.webp",
    logo_alt: "Blue Wallet logo",
    faq_dir: "bluewallet",
    downloads: &[
        DownloadLink {
            label: "Google Play",
            url: "https://play.google.com/store/apps/details?id=io.bluewallet.bluewallet",
            logo: "/google_play.png",
            logo_alt: "Google Play",
            platforms: &["android"],
        },
        DownloadLink {
            label: "GitHub APK",
            url: "https://github.com/BlueWallet/BlueWallet/releases",
            logo: "/GitHub_Logo.png",
            logo_alt: "GitHub",
            platforms: &["android"],
        },
        DownloadLink {
            label: "App Store",
            url: "https://apps.apple.com/app/bluewallet-bitcoin-wallet/id1376878040",
            logo: "/download_on_app_store.png",
            logo_alt: "App Store",
            platforms: &["ios"],
        },
    ],
};

pub static GREEN_WALLET: WalletDef = WalletDef {
    id: "green",
    name: "Blockstream Green Wallet",
    tagline: "More Powerful Than Ever",
    description: "Blockstream Green is an industry-leading Bitcoin wallet that offers you an unrivaled blend of security and ease-of-use.",
    color: "#038046",
    logo: "/green_logo.webp",
    logo_alt: "Green Wallet logo",
    faq_dir: "greenwallet",
    downloads: &[
        DownloadLink {
            label: "Google Play",
            url: "https://play.google.com/store/apps/details?id=com.greenaddress.greenbits_android_wallet",
            logo: "/google_play.png",
            logo_alt: "Google Play",
            platforms: &["android"],
        },
        DownloadLink {
            label: "GitHub APK",
            url: "https://github.com/Blockstream/green_android/releases",
            logo: "/GitHub_Logo.png",
            logo_alt: "GitHub",
            platforms: &["android"],
        },
        DownloadLink {
            label: "App Store",
            url: "https://apps.apple.com/us/app/green-bitcoin-wallet/id1402243590",
            logo: "/download_on_app_store.png",
            logo_alt: "App Store",
            platforms: &["ios"],
        },
    ],
};

pub static SPARROW_WALLET: WalletDef = WalletDef {
    id: "sparrow",
    name: "Sparrow Wallet",
    tagline: "Gain Financial Sovereignty with Sparrow Wallet.",
    description: "",
    color: "#6f767c",
    logo: "/sparrow.png",
    logo_alt: "Sparrow Wallet logo",
    faq_dir: "sparrow",
    downloads: &[DownloadLink {
        label: "Download Sparrow",
        url: "https://sparrowwallet.com/download/",
        logo: "/download_sparrow.png",
        logo_alt: "Download Sparrow Wallet",
        platforms: &["desktop"],
    }],
};

pub static ALL_WALLETS: &[&WalletDef] =
    &[&BLUE_WALLET, &GREEN_WALLET, &SPARROW_WALLET];

pub fn find_wallet(id: &str) -> Option<&'static WalletDef> {
    ALL_WALLETS.iter().find(|w| w.id == id).copied()
}

// =============================================================================
// GUIDE LEVEL DEFINITIONS
// =============================================================================

pub static BASIC_LEVEL: GuideLevelDef = GuideLevelDef {
    id: "basic",
    name: "Basic",
    subtitle: "I have a teeny weeny stack",
    title: "Basic Self-Custody Guide",
    quote: "Trusted Third Parties are Security Holes",
    quote_author: "-Nick Szabo",
    intro: "This basic setup is meant to get you up to speed quickly. You'll pick one of the wallets below, create your private key and take possession of your bitcoin. I wouldn't recommend storing too much of your wealth in a mobile wallet. Think of it as a self-custodied spending wallet, similar to how you'd carry cash in a physical wallet.",
    platforms: &["android", "ios", "desktop"],
    wallets: &["blue", "green", "sparrow"],
    faq_dir: None,
    products: &[],
    steps: &[],
};

pub static INTERMEDIATE_LEVEL: GuideLevelDef = GuideLevelDef {
    id: "intermediate",
    name: "Intermediate",
    subtitle: "I have an average stack",
    title: "Intermediate Self-Custody Guide",
    quote: "Rights Are Not Given, They Are Taken",
    quote_author: "-Aldous Huxley",
    intro: "It's time to take your bitcoin privacy and security to the next level. In this guide we'll build on our previous basic desktop setup.",
    platforms: &["desktop"],
    wallets: &[],
    faq_dir: None,
    products: &[],
    steps: &[
        GuideStep {
            id: "hardware-wallet",
            name: "Hardware Wallet",
            title: "Step 1 - Hardware Wallet Setup",
            icon: "/increase.png",
            icon_alt: "Level up arrow",
            faq_dir: "hardware_wallet_setup",
            products: &[
                ProductLink {
                    name: "Buy a ColdCard",
                    description: "",
                    url: "https://store.coinkite.com/store/bundle-mk4-basic",
                    logo: "/coldcard-logo-nav.png",
                    logo_alt: "Coldcard logo",
                    logo_width: "24",
                    logo_height: "8",
                },
                ProductLink {
                    name: "Buy a Seedplate",
                    description: "",
                    url: "https://store.coinkite.com/store/seedplate",
                    logo: "/steel.png",
                    logo_alt: "Steel plate",
                    logo_width: "10",
                    logo_height: "8",
                },
                ProductLink {
                    name: "Buy a Center Punch",
                    description: "",
                    url: "https://store.coinkite.com/store/drillpunch",
                    logo: "/hole-puncher.png",
                    logo_alt: "Hole puncher",
                    logo_width: "10",
                    logo_height: "8",
                },
            ],
            next_step: Some("node"),
            next_step_label: Some("Step 2 - Node Setup"),
            next_step_button_label: Some("Running Bitcoin"),
        },
        GuideStep {
            id: "node",
            name: "Node Setup",
            title: "Step 2 - Node Setup",
            icon: "/bitcoin_server.png",
            icon_alt: "Bitcoin server",
            faq_dir: "node_setup",
            products: &[
                ProductLink {
                    name: "Sovereign Computing",
                    description: "",
                    url: "https://start9.com/",
                    logo: "/start9_transparent_inverted.png",
                    logo_alt: "Start9 logo",
                    logo_width: "28",
                    logo_height: "8",
                },
                ProductLink {
                    name: "Bitcoin, Lightning and more!",
                    description: "",
                    url: "https://mynodebtc.github.io/",
                    logo: "/mynode_logo.png",
                    logo_alt: "MyNode logo",
                    logo_width: "30",
                    logo_height: "8",
                },
                ProductLink {
                    name: "Not Your Node, Not your Rules",
                    description: "",
                    url: "https://shop.fulmo.org/raspiblitz/",
                    logo: "/raspiblitz_logo_main.png",
                    logo_alt: "RaspiBlitz logo",
                    logo_width: "28",
                    logo_height: "8",
                },
            ],
            next_step: None,
            next_step_label: None,
            next_step_button_label: None,
        },
    ],
};

pub static ADVANCED_LEVEL: GuideLevelDef = GuideLevelDef {
    id: "advanced",
    name: "Advanced",
    subtitle: "I am well equipped",
    title: "Advanced Self-Custody Guide",
    quote: "",
    quote_author: "",
    intro: "Taking self-custody of your bitcoin comes with great responsibility, especially when that bitcoin could become generational wealth, therefore it is wise to take extra precautions. That being said, we should take care to keep things as simple as possible, while also ensuring a high degree of privacy and security.",
    platforms: &["desktop"],
    wallets: &[],
    faq_dir: Some("advanced_desktop_setup"),
    products: &[],
    steps: &[],
};

pub static ALL_LEVELS: &[&GuideLevelDef] =
    &[&BASIC_LEVEL, &INTERMEDIATE_LEVEL, &ADVANCED_LEVEL];

pub fn find_level(id: &str) -> Option<&'static GuideLevelDef> {
    ALL_LEVELS.iter().find(|l| l.id == id).copied()
}

/// Get wallets available for a given level and platform.
pub fn wallets_for(
    level: &GuideLevelDef,
    platform: &str,
) -> Vec<&'static WalletDef> {
    level
        .wallets
        .iter()
        .filter_map(|wid| find_wallet(wid))
        .filter(|w| {
            // Wallet is available on this platform if any of its downloads target it
            w.downloads.iter().any(|d| d.platforms.contains(&platform))
        })
        .collect()
}

/// Get downloads for a wallet filtered to a specific platform.
pub fn downloads_for(
    wallet: &WalletDef,
    platform: &str,
) -> Vec<&'static DownloadLink> {
    wallet
        .downloads
        .iter()
        .filter(|d| d.platforms.contains(&platform))
        .collect()
}
