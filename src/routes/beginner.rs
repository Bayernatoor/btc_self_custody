use leptos::{ev::MouseEvent, *};

use crate::extras::accordion_menu::AccordionMenu;
use crate::helpers::get_path::get_current_path;

#[derive(Clone, Copy)]
pub enum WalletName {
    Green,
    Blue,
    Sparrow,
}

// FIXME: refactor all buttons, add them to generic_button module.
// to much repetition and different buttons being used.:

#[component]
#[allow(non_snake_case)]
pub fn WalletButton<F>(
    on_click: F,
    wallet_title: String,
    short_desc: String,
    img_url: String,
    img_alt: String,
    text_color: String,
    selected_wallet: WalletName,
    platform: String,
) -> impl IntoView
where
    F: Fn(MouseEvent) + 'static,
{
    // determine which wallet button was clicked on
    let (wallet, set_wallet) = create_signal(String::new());

    match selected_wallet {
        WalletName::Green => set_wallet("green".to_string()),
        WalletName::Blue => set_wallet("blue".to_string()),
        WalletName::Sparrow => set_wallet("sparrow".to_string()),
    }

    // get the name of the wallet
    let wallet_name = wallet();
    // create our url path
    let path = format!("/guides/basic/{platform}/{wallet_name}");

    view! {
        <a href=path>
            <button
                class="flex justify-center shrink-0 h-20 w-72 p-4 mx-auto bg-white rounded-xl items-center space-x-4 hover:bg-[#f2f2f2] shadow-inner"
                on:click=on_click
            >
                <div class="shrink-0">
                    <img class="h-12 w-12 rounded-md" src=img_url alt=img_alt/>
                </div>
                <div>
                    <h3 class=format!("text-xl font-medium text-[{text_color}]")>{wallet_title}</h3>
                    <p class="text-slate-500">{short_desc}</p>
                </div>
            </button>
        </a>
    }
}

#[component]
#[allow(non_snake_case)]
pub fn DownloadButton(
    href: String,
    logo: String,
    alt_txt: String,
) -> impl IntoView {
    view! {
        <a
            href=href
            rel="noreferrer"
            target="_blank"
            rel="noreferrer"
            class="flex h-18 w-64 rounded-xl"
        >
            <button class="flex justify-center p-2 mx-auto bg-white items-center rounded-xl">
                <div class="flex justify-center h-full w-full">
                    <img
                        class="max-h-full max-w-full object-contain"
                        src=format!("{}", logo)
                        alt=format!("{}", alt_txt)
                    />
                </div>
            </button>
        </a>
    }
}

/// Component used for mobile (basic) pages.
#[component]
#[allow(non_snake_case)]
pub fn BeginnerPageTemplate(
    title: String,
    quote: String,
    quote_author: String,
    intro: String,
    #[prop(optional)] wallet_one_text: String,
    #[prop(optional)] wallet_two_text: String,
) -> impl IntoView {
    // used for onlick to determine which button was clicked
    let (_green_clicked, set_green_clicked) = create_signal(false);
    let (_blue_clicked, set_blue_clicked) = create_signal(false);

    // set the button details
    let (_green_details, set_green_details) = create_signal(false);
    let (_blue_details, set_blue_details) = create_signal(false);

    // get current path via RouteContext
    let path = get_current_path();
    let (platform, set_platform) = create_signal(String::new());

    if path.contains("ios") {
        set_platform("ios".to_string());
    } else if path.contains("android") {
        set_platform("android".to_string());
    } else {
        set_platform("desktop".to_string());
    }

    // Green wallet assets
    let wallet_name_green = "Green Wallet".to_string();
    let short_desc_green = "Feature Rich Wallet".to_string();
    let img_url_green = "./../../green_logo.webp".to_string();
    let img_alt_green = "Green Logo".to_string();
    let text_color_green = "#038046".to_string();

    // Blue wallet assets
    let wallet_name_blue = "Blue Wallet".to_string();
    let short_desc_blue = "Basic + Ease of Use".to_string();
    let img_url_blue = "./../../bluewallet_logo.webp".to_string();
    let img_alt_blue = "Blue Wallet".to_string();
    let text_color_blue = "#1a578f".to_string();

    // renders the guides/basic/* route
    view! {
        <div
            id="basic"
            class="grid gap-6 max-w-3xl mx-auto mt-8 mb-24 animate-fadeinone grid-rows-[auto_auto_1fr] lg:max-w-4xl xl:max-w-5xl lg:gap-8 md:my-28"
        >
            // Section 1: Title, Quote, and Quote Author
            <div class="mt-10 lg:mt-0 px-6">
                <h1 class="text-center text-[2.25rem] text-[#f7931a] font-semibold md:text-[2.5rem] lg:text-[3rem]">
                    {title}
                </h1>
                <div class="text-center max-w-sm mx-auto">
                    <p class="text-lg text-white italic">{quote}</p>
                </div>
                <div class="text-center max-w-sm mx-auto">
                    <p class="text-sm text-white italic">{quote_author}</p>
                </div>
            </div>

            // Section 2: Intro
            <div class="px-6 pt-4 lg:pt-0 lg:px-0">
                <p class="text-xl font-semibold text-white pb-2 ">
                    "Bitcoin Self-Custody: The act of taking possession of a bitcoin private key."
                </p>
                <p class="text-lg text-white pb-2">{intro}</p>
                <p class="text-lg text-[#f7931a]">
                    <strong>{wallet_name_blue.clone()}</strong>
                    <p class="text-lg text-white" inner_html=wallet_one_text></p>
                </p>
                <p class="text-lg text-[#f7931a] pt-2">
                    <strong>{wallet_name_green.clone()}</strong>
                    <p class="text-lg text-white" inner_html=wallet_two_text></p>
                </p>
            </div>

            // Section 3: Everything Else
            <div class="px-6 lg:pb-4 lg:px-0">
                <hr class="border border-solid border-gray-400 mx-auto w-full mb-6"/>

                <h2 class="text-center pb-4 text-[1.5rem] font-semibold text-[#f7931a] font-semibold">
                    "Pick A Wallet"
                </h2>

                <div class="flex flex-col mx-auto justify-center lg:flex-row px-6 gap-4">
                    <WalletButton
                        on_click=move |_| {
                            set_blue_clicked(true);
                            set_blue_details(true);
                        }

                        selected_wallet=WalletName::Blue
                        platform=platform()
                        wallet_title=wallet_name_blue.clone()
                        short_desc=short_desc_blue.clone()
                        img_url=img_url_blue.clone()
                        img_alt=img_alt_blue.clone()
                        text_color=text_color_blue.clone()
                    />
                    <WalletButton
                        on_click=move |_| {
                            set_green_clicked(true);
                            set_green_details(true);
                        }

                        selected_wallet=WalletName::Green
                        platform=platform()
                        wallet_title=wallet_name_green.clone()
                        short_desc=short_desc_green.clone()
                        img_url=img_url_green.clone()
                        img_alt=img_alt_green.clone()
                        text_color=text_color_green.clone()
                    />
                </div>
            </div>
        </div>
    }
}

/// Component used for beginner (basic) desktop pages.
#[component]
#[allow(non_snake_case)]
pub fn BeginnerDesktopPageTemplate(
    title: String,
    quote: String,
    quote_author: String,
    intro: String,
) -> impl IntoView {
    // used for onlick to determine which button was clicked
    let (_sparrow_clicked, set_sparrow_clicked) = create_signal(false);

    // Sparrow wallet assets
    let wallet_name_sparrow = "Sparrow Wallet".to_string();
    let short_desc_sparrow = "Financial self sovereignty".to_string();
    let img_url_sparrow = "./../../sparrow.png".to_string();
    let img_alt_sparrow = "sparrow wallet image".to_string();
    let text_color_sparrow = "#6f767c".to_string();

    view! {
        <div
            id="basic"
            class="grid gap-6 mx-auto max-w-3xl mt-8 mb-24 rounded-xl animate-fadeinone grid-rows-[auto_auto_1fr] lg:max-w-4xl xl:max-w-5xl lg:gap-8 md:my-28"
        >
            // Section 1: Title, Quote, and Quote Author
            <div class="mt-10 lg:mt-0">
                <h1 class="text-center text-[2.25rem] px-6 text-[#f7931a] font-semibold md:text-[2.5rem] lg:text-[3rem] lg:px-0">
                    {title}
                </h1>
                <div class="text-center max-w-sm mx-auto">
                    <p class="text-lg text-white italic">{quote}</p>
                </div>
                <div class="text-center max-w-sm mx-auto">
                    <p class="text-sm text-white italic">{quote_author}</p>
                </div>
            </div>

            // Section 2: Intro
            <div class="px-6 pt-4 lg:pt-0 lg:px-0">
                <p class="text-lg text-white">{intro}</p>
            </div>

            // Section 3: Everything Else
            <div class="px-6 lg:pb-4 lg:px-0">
                <hr class="border border-solid border-gray-400 mx-auto mb-6 w-full"/>

                <h2 class="text-center pb-4 text-xl font-semibold text-[#f7931a]">
                    "Recommended Wallet"
                </h2>

                <div class="flex flex-col justify-center lg:flex-row px-6 py-2 gap-4 max-w-2xl mx-auto">
                    <WalletButton
                        on_click=move |_| {
                            set_sparrow_clicked(true);
                        }

                        selected_wallet=WalletName::Sparrow
                        platform="desktop".to_string()
                        wallet_title=wallet_name_sparrow.clone()
                        short_desc=short_desc_sparrow.clone()
                        img_url=img_url_sparrow.clone()
                        img_alt=img_alt_sparrow.clone()
                        text_color=text_color_sparrow.clone()
                    />
                </div>
            </div>
        </div>
    }
}

/// Renders the basic Android page
/// This comp should be reviewed and is likely redundant.
#[component]
#[allow(non_snake_case)]
pub fn RenderAndroidPage() -> impl IntoView {
    let intro_text: String = "This basic Android setup is meant to get you up to speed quickly. You'll pick one of the one wallets
        below, create your private key and take possession of your bitcoin. I wouldn't recommend storing too much of your wealth in a
        mobile wallet. Think of it as a self-custodied spending wallet, similar to how you'd carry cash in a physical wallet.
        ".to_string();

    let wallet_one_text: String = " is a tried and tested On-chain Bitcoin wallet. It's easy to setup, follows all the latest standards, has great features such as: multiple 
        wallet creation, Multisig Vaults, duress wallet capability, payjoins and of course the ability to
        connect to your own Electrum or Lightning Node.".to_string();

    let wallet_two_text: String = " is an easy to use self-custodial On-chain Bitcoin wallet built by Blockstream. It has many advanced feaures such as: Multi-Signature wallets with 2FA, Multi wallet creation, the ability to connect your own Electrum node, and access to a Bitcoin layer 2 called the 
        <a class='text-[#8cb4ff] underline-offset-auto' href='https://blockstream.com/liquid/' target='_blank' rel='noopener noreferrer'>
            Liquid Network.
        </a>
        ."
    .to_string();

    let title = "Basic Android Self-Custody Guide".to_string();
    let quote = "Trusted Third Parties are Security Holes".to_string();
    let quote_author = "-Nick Szabo".to_string();

    view! {
        <BeginnerPageTemplate
            title=title
            quote=quote
            quote_author=quote_author
            intro=intro_text
            wallet_one_text=wallet_one_text
            wallet_two_text=wallet_two_text
        />
    }
}

/// Renders the basic IOS page.
#[component]
#[allow(non_snake_case)]
pub fn RenderIosPage() -> impl IntoView {
    let intro_text: String = "This basic iOS setup is meant to get you up to speed quickly. You'll pick one of the one wallets
        below, create your private key and take posession of your bitcoin. I wouldn't recommend storing too much of your wealth in a
        mobile wallet. Think of it as a self-custodied spending wallet, similar to how you'd carry cash in a physical wallet.
        ".to_string();

    let wallet_one_text: String = " is a tried and tested On-chain Bitcoin wallet. It's easy to setup, follows all the latest standards, has great features such as: Multiple 
        Wallet Creation, Multisig Vaults, Duress Wallet capability, Payjoins and of course the ability to
        connect to your own Electrum or Lightning Node.".to_string();

    let wallet_two_text: String = " is an easy to use self-custodial On-chain Bitcoin wallet built by Blockstream. It has many advanced feaures such as: Multi-Signature 
        wallets with 2FA, Multi wallet creation, the ability to connect your own Electrum node, and access to a Bitcoin layer 2 called the 
        <a class='text-[#8cb4ff] underline-offset-auto' href='https://blockstream.com/liquid/' target='_blank' rel='noopener noreferrer'>
            Liquid Network
        </a>."
    .to_string();

    let title = "Basic iOS Self-Custody Guide".to_string();
    let quote = "Trusted Third Parties are Security Holes".to_string();
    let quote_author = "-Nick Szabo".to_string();

    view! {
        <BeginnerPageTemplate
            title=title
            quote=quote
            quote_author=quote_author
            intro=intro_text
            wallet_one_text=wallet_one_text
            wallet_two_text=wallet_two_text
        />
    }
}

/// Renders the basic desktop page.
#[component]
#[allow(non_snake_case)]
pub fn RenderDesktopPage() -> impl IntoView {
    let intro_text: String = "Desktop wallets, such as Sparrow Wallet, deliver heightened security versus mobile options. 
        Often employed in elaborate setups for self-custodying sizeable Bitcoin savings, they remain accessible even for basic use cases. 
        This guide begins with a simplified configuration, expanding upon it later. Ideal for individuals intending to grow their Bitcoin holdings, 
        this introduction sets the stage for more advanced techniques found in intermediate and advanced guides.".to_string();

    let title = "Basic Desktop Self-Custody Guide".to_string();
    let quote = "Trusted Third Parties are Security Holes".to_string();
    let quote_author = "-Nick Szabo".to_string();

    view! {
        <BeginnerDesktopPageTemplate
            title=title
            quote=quote
            quote_author=quote_author
            intro=intro_text
        />
    }
}
/// Route for the android instructions - renders either bluewallet or greenwallet
/// depends on button clicked.
#[component]
#[allow(non_snake_case)]
pub fn BeginnerWalletInstructions(
    selected_wallet: WalletName,
    ios: bool,
) -> impl IntoView {
    let google_play_logo = "./../../../google_play.png".to_string();
    let google_play_alt = "Google Play Logo".to_string();

    let apple_store_logo = "./../../../download_on_app_store.png".to_string();
    let apple_store_alt = "Apple Store Logo".to_string();

    let img_url_github = "./../../../GitHub_Logo.png".to_string();
    let img_alt_github = "Github Logo".to_string();

    // Blue wallet assest
    let blue_google_play =
        r"https://play.google.com/store/apps/details?id=io.bluewallet.bluewallet".to_string();
    let blue_apple_store =
        r"https://apps.apple.com/app/bluewallet-bitcoin-wallet/id1376878040"
            .to_string();
    let blue_android_apk =
        r"https://github.com/BlueWallet/BlueWallet/releases".to_string();
    // Green wallet assest
    let green_google_play =
        r"https://play.google.com/store/apps/details?id=com.greenaddress.greenbits_android_wallet"
            .to_string();
    let green_android_apk =
        r"https://github.com/Blockstream/green_android/releases".to_string();

    let green_apple_store =
        r"https://apps.apple.com/us/app/green-bitcoin-wallet/id1402243590"
            .to_string();
    // Sparrow wallet assest
    let sparrow_download = r"https://sparrowwallet.com/download/".to_string();
    let img_url_sparrow = "./../../../download_sparrow.png".to_string();
    let img_alt_sparrow = "download sparrow wallet".to_string();

    let (displayed_wallet, set_displayed_wallet) = create_signal("");

    match selected_wallet {
        WalletName::Green => set_displayed_wallet("green"),
        WalletName::Blue => set_displayed_wallet("blue"),
        WalletName::Sparrow => set_displayed_wallet("sparrow"),
    }

    if displayed_wallet() == "blue" {
        view! {
            <div
                id="basic"
                class="grid gap-6 max-w-3xl mx-auto mt-8 mb-24 animate-fadeinone grid-rows-[auto_auto_1fr] lg:max-w-4xl xl:max-w-5xl lg:gap-8 md:my-28"
            >
                // Section 1: Title, Quote, and Quote Author
                <div class="lg:mt-0 px-6">
                    <h1 class="text-center text-[2.25rem] font-semibold text-[#f7931a] md:text-[2.5rem] lg:text-[3rem]">
                        "Blue Wallet"
                    </h1>
                    <div class="text-center mx-auto">
                        <p class="text-lg font-semibold text-white italic">
                            "Radically Simple 👩‍🎤 Extremely Powerful."
                        </p>
                    </div>
                    <div class="text-center mx-auto">
                        <p class="text-md text-white italic">
                            "A freedom and self-sovereign tool, disguised as a cute little Blue app in your pocket."
                        </p>
                    </div>
                </div>

                // Section 2: Download Options
                <div class="flex flex-col mx-auto justify-center px-6 py-2 max-w-2xl mx-auto gap-4">
                    <Show
                        when=move || ios
                        fallback=move || {
                            view! {
                                <DownloadButton
                                    href=blue_google_play.clone()
                                    logo=google_play_logo.clone()
                                    alt_txt=google_play_alt.clone()
                                />
                                <DownloadButton
                                    href=blue_android_apk.clone()
                                    logo=img_url_github.clone()
                                    alt_txt=img_alt_github.clone()
                                />
                            }
                        }
                    >

                        <DownloadButton
                            href=blue_apple_store.clone()
                            logo=apple_store_logo.clone()
                            alt_txt=apple_store_alt.clone()
                        />
                    </Show>
                </div>

                // Section 3: Start Here
                <div class="mx-auto max-w-5xl p-4 w-full">
                    <div class="mx-auto border border-solid border-gray-400"></div>
                    <h2 class="flex justify-center font-semibold text-[#f7931a] text-[1.5rem] pt-6 pb-4">
                        "Get Started"
                    </h2>
                    // Renders FAQs menu
                    <AccordionMenu faq_name="bluewallet".to_string()/>
                </div>
            </div>
        }
    } else if displayed_wallet() == "green" {
        view! {
            <div
                id="basic"
                class="grid gap-6 max-w-3xl mx-auto mt-8 mb-24 animate-fadeinone grid-rows-[auto_auto_1fr] lg:max-w-4xl xl:max-w-5xl lg:gap-8 md:my-28"
            >
                // Section 1: Title, Quote, and Quote Author
                <div class="lg:mt-0 px-6">
                    <h1 class="text-center text-[2.25rem] font-semibold text-[#f7931a] md:text-[2.5rem] lg:text-[3rem]">
                        "Blockstream Green Wallet"
                    </h1>
                    <div class="text-center mx-auto">
                        <p class="text-lg font-semibold text-white italic">
                            "More Powerful Than Ever"
                        </p>
                    </div>
                    <div class="text-center mx-auto">
                        <p class="text-md text-white italic">
                            "Blockstream Green is an industry-leading Bitcoin wallet that offers you an unrivaled blend of security and ease-of-use."
                        </p>
                    </div>
                </div>

                // Section 2: Download Options
                <div class="flex flex-col mx-auto justify-center px-6 py-2 max-w-2xl mx-auto gap-4">
                    <Show
                        when=move || ios
                        fallback=move || {
                            view! {
                                <DownloadButton
                                    href=green_google_play.clone()
                                    logo=google_play_logo.clone()
                                    alt_txt=google_play_alt.clone()
                                />
                                <DownloadButton
                                    href=green_android_apk.clone()
                                    logo=img_url_github.clone()
                                    alt_txt=img_alt_github.clone()
                                />
                            }
                        }
                    >

                        <DownloadButton
                            href=green_apple_store.clone()
                            logo=apple_store_logo.clone()
                            alt_txt=apple_store_alt.clone()
                        />
                    </Show>
                </div>

                // Section 3: Get Started
                <div class="mx-auto max-w-5xl p-4 w-full">
                    <div class="mx-auto border border-solid border-gray-400"></div>
                    <h2 class="flex justify-center font-semibold text-[#f7931a] text-[1.5rem] pt-6 pb-4">
                        "Get Started"
                    </h2>
                    // Renders FAQs menu
                    <AccordionMenu faq_name="greenwallet".to_string()/>
                </div>
            </div>
        }
    } else {
        view! {
            <div
                id="basic"
                class="grid gap-6 max-w-3xl mx-auto mt-8 mb-24 animate-fadeinone grid-rows-[auto_auto_1fr] lg:max-w-4xl xl:max-w-5xl lg:gap-8 md:my-28"
            >
                // Section 1: Title, Quote, and Quote Author
                <div class="lg:mt-0 px-6">
                    <h1 class="text-center text-[2.25rem] font-semibold text-[#f7931a] md:text-[2.5rem] lg:text-[3rem]">
                        "Sparrow Wallet"
                    </h1>
                    <div class="text-center mx-auto">
                        <p class="text-white text-lg font-semibold px-4">
                            "Gain Financial Sovereignty with Sparrow Wallet."
                        </p>
                    </div>
                </div>

                // Section 2: Download Options
                <div class="flex flex-col mx-auto justify-center px-6 py-2 mx-auto gap-4 lg:px-4">
                    <DownloadButton
                        href=sparrow_download.clone()
                        logo=img_url_sparrow.clone()
                        alt_txt=img_alt_sparrow.clone()
                    />
                </div>

                // Section 3: Get Started
                <div class="mx-auto max-w-5xl p-4 w-full">
                    <div class="mx-auto border border-solid border-gray-400"></div>
                    <h2 class="flex justify-center font-semibold text-[#f7931a] text-[1.5rem] pt-6 pb-4">
                        "Get Started"
                    </h2>
                    // Renders FAQs menu
                    <AccordionMenu faq_name="sparrow".to_string()/>
                </div>
            </div>
        }
    }
}
