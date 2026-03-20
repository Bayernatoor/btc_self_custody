//! Guide definitions: all wallet, level, and platform metadata lives here.
//!
//! Single source of truth - no guide content is hardcoded in route components.

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
    /// Brand color for the download button
    pub color: &'static str,
    /// SVG icon path for the button (inline)
    pub icon: &'static str,
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
            color: "#01875f",
            icon: r#"<path d="M3.609 1.814L13.792 12 3.61 22.186a.996.996 0 01-.61-.92V2.734a1 1 0 01.609-.92zm10.89 10.893l2.302 2.302-10.937 6.333 8.635-8.635zm3.199-1.4l2.834 1.64a1 1 0 010 1.74l-2.834 1.64-2.532-2.534 2.532-2.486zM5.864 2.658L16.8 8.99l-2.302 2.302-8.635-8.635z"/>"#,
            platforms: &["android"],
        },
        DownloadLink {
            label: "GitHub Releases",
            url: "https://github.com/BlueWallet/BlueWallet/releases",
            logo: "/GitHub_Logo.png",
            logo_alt: "GitHub",
            color: "#24292f",
            icon: r#"<path fill-rule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clip-rule="evenodd"/>"#,
            platforms: &["android"],
        },
        DownloadLink {
            label: "App Store",
            url: "https://apps.apple.com/app/bluewallet-bitcoin-wallet/id1376878040",
            logo: "/download_on_app_store.png",
            logo_alt: "App Store",
            color: "#0071e3",
            icon: r#"<path d="M18.71 19.5c-.83 1.24-1.71 2.45-3.05 2.47-1.34.03-1.77-.79-3.29-.79-1.53 0-2 .77-3.27.82-1.31.05-2.3-1.32-3.14-2.53C4.25 17 2.94 12.45 4.7 9.39c.87-1.52 2.43-2.48 4.12-2.51 1.28-.02 2.5.87 3.29.87.78 0 2.26-1.07 3.8-.91.65.03 2.47.26 3.64 1.98-.09.06-2.17 1.28-2.15 3.81.03 3.02 2.65 4.03 2.68 4.04-.03.07-.42 1.44-1.38 2.83M13 3.5c.73-.83 1.94-1.46 2.94-1.5.13 1.17-.34 2.35-1.04 3.19-.69.85-1.83 1.51-2.95 1.42-.15-1.15.41-2.35 1.05-3.11z"/>"#,
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
            color: "#01875f",
            icon: r#"<path d="M3.609 1.814L13.792 12 3.61 22.186a.996.996 0 01-.61-.92V2.734a1 1 0 01.609-.92zm10.89 10.893l2.302 2.302-10.937 6.333 8.635-8.635zm3.199-1.4l2.834 1.64a1 1 0 010 1.74l-2.834 1.64-2.532-2.534 2.532-2.486zM5.864 2.658L16.8 8.99l-2.302 2.302-8.635-8.635z"/>"#,
            platforms: &["android"],
        },
        DownloadLink {
            label: "GitHub Releases",
            url: "https://github.com/Blockstream/green_android/releases",
            logo: "/GitHub_Logo.png",
            logo_alt: "GitHub",
            color: "#24292f",
            icon: r#"<path fill-rule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clip-rule="evenodd"/>"#,
            platforms: &["android"],
        },
        DownloadLink {
            label: "App Store",
            url: "https://apps.apple.com/us/app/green-bitcoin-wallet/id1402243590",
            logo: "/download_on_app_store.png",
            logo_alt: "App Store",
            color: "#0071e3",
            icon: r#"<path d="M18.71 19.5c-.83 1.24-1.71 2.45-3.05 2.47-1.34.03-1.77-.79-3.29-.79-1.53 0-2 .77-3.27.82-1.31.05-2.3-1.32-3.14-2.53C4.25 17 2.94 12.45 4.7 9.39c.87-1.52 2.43-2.48 4.12-2.51 1.28-.02 2.5.87 3.29.87.78 0 2.26-1.07 3.8-.91.65.03 2.47.26 3.64 1.98-.09.06-2.17 1.28-2.15 3.81.03 3.02 2.65 4.03 2.68 4.04-.03.07-.42 1.44-1.38 2.83M13 3.5c.73-.83 1.94-1.46 2.94-1.5.13 1.17-.34 2.35-1.04 3.19-.69.85-1.83 1.51-2.95 1.42-.15-1.15.41-2.35 1.05-3.11z"/>"#,
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
        color: "#6f767c",
        icon: r#"<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"/>"#,
        platforms: &["desktop-linux", "desktop-macos", "desktop-windows"],
    }],
};

/// Desktop sub-platforms (OS options shown when "Desktop" is selected).
pub static DESKTOP_OS: &[(&str, &str)] = &[
    ("desktop-linux", "Linux"),
    ("desktop-macos", "macOS"),
    ("desktop-windows", "Windows"),
];

/// OS-specific tips shown at the top of desktop guide pages.
pub fn os_tip(platform: &str) -> Option<(&'static str, &'static str)> {
    match platform {
        "desktop-linux" => Some((
            "Linux Tip",
            "Sparrow is distributed as an AppImage or .deb package. You may need to set executable permissions: right-click the file → Properties → Permissions → Allow executing, or run chmod +x in your terminal. If using the .deb, install with sudo dpkg -i."
        )),
        "desktop-macos" => Some((
            "macOS Tip",
            "Sparrow is distributed as a .dmg file. After opening, drag Sparrow to your Applications folder. On first launch, macOS may block it - go to System Settings → Privacy & Security and click Open Anyway."
        )),
        "desktop-windows" => Some((
            "Windows Tip",
            "Sparrow is distributed as an .exe installer. Windows Defender may flag it on first run - click More Info → Run Anyway. Always verify the download signature to ensure it hasn't been tampered with."
        )),
        _ => None,
    }
}

/// Check if a platform is a desktop OS variant.
pub fn is_desktop_os(platform: &str) -> bool {
    platform.starts_with("desktop-")
}

/// Get display name for any platform.
pub fn platform_display(platform: &str) -> &str {
    match platform {
        "android" => "Android",
        "ios" => "iOS",
        "desktop" => "Desktop",
        "desktop-linux" => "Linux",
        "desktop-macos" => "macOS",
        "desktop-windows" => "Windows",
        p => p,
    }
}

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
    quote_author: "- Nick Szabo",
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
    quote_author: "- Aldous Huxley",
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
                    name: "Not Your Node, Not Your Rules",
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
