use leptos::{ev::MouseEvent, *};

use crate::extras::accordion_menu::AccordionMenu;
use crate::helpers::get_path::get_current_path;

#[derive(Clone, Copy)]
pub enum WalletName {
    Mutiny,
    Blue,
    Sparrow,
}

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
        WalletName::Mutiny => set_wallet("mutiny".to_string()),
        WalletName::Blue => set_wallet("blue".to_string()),
        WalletName::Sparrow => set_wallet("sparrow".to_string()),
    }

    // get the name of the wallet
    let wallet_name = wallet();
    // create our url path
    let path = format!("/guides/basic/{platform}/{wallet_name}");

    view! {
        <a href=path>
            <button class="flex justify-center shrink-0 h-20 w-72 p-4 mx-auto bg-white rounded-xl items-center space-x-4 hover:bg-[#f2f2f2] shadow-inner" on:click=on_click>
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
    #[prop(optional)] button_name: Option<String>,
) -> impl IntoView {
    let (button, set_button) = create_signal(String::new());
    let (width, set_width) = create_signal(12);
    let (button_width, set_button_width) = create_signal(64);
    let (basis_one, set_basis_one) = create_signal("1/3".to_string());
    let (basis_two, set_basis_two) = create_signal("2/3".to_string());
    let (flex_justify, set_flex_justify) = create_signal(String::new());

    let name = match button_name.clone() {
        Some(name) => name,
        None => "No Button Name".to_string(),
    };

    set_button(name.clone());

    if button_name.is_none() {
        set_width(48);
        set_button_width(48);
        set_flex_justify("justify-center".to_string());
        set_basis_one("full".to_string());
        set_basis_two("full".to_string());
    }
    view! {
        <a href=href rel="noreferrer" target="_blank" rel="noreferrer" class="flex h-18 w-72">
            <button class=format!("flex {} h-auto w-{} p-2 mx-auto bg-white rounded-xl items-center hover:bg-[#f2f2f2]",
                                  flex_justify.get_untracked(), button_width.get_untracked())>
              <div class=format!("flex justify-center basis-{}", basis_one.get_untracked())>
                <img class=format!("h-auto w-{}", width.get()) src=format!("{}", logo) alt=format!("{}", alt_txt) />
              </div>
                <Show
                    when=move || button_name.is_some()
                    fallback=move || view!("")>
                    <div class="">
                        <div class=format!("basis-{}", basis_two.get_untracked())>
                          <p class=format!("text-[1.25rem] font-bold text-black")>{button().to_string()}</p>
                        </div>
                    </div>
                </Show>
            </button>
        </a>
    }
}

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
    let (_mutiny_clicked, set_mutiny_clicked) = create_signal(false);
    let (_blue_clicked, set_blue_clicked) = create_signal(false);

    // set the button details
    let (_mutiny_details, set_mutiny_details) = create_signal(false);
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

    // Mutiny wallet assets
    let wallet_name_mutiny = "Mutiny Wallet".to_string();
    let short_desc_mutiny = "On-chain + Lightning".to_string();
    let img_url_mutiny = "./../../mutiny_logo.webp".to_string();
    let img_alt_mutiny = "Mutiny Logo".to_string();
    let text_color_mutiny = "#f71d5a".to_string();

    // Blue wallet assets
    let wallet_name_blue = "Blue Wallet".to_string();
    let short_desc_blue = "Basic + Ease of Use".to_string();
    let img_url_blue = "./../../bluewallet_logo.webp".to_string();
    let img_alt_blue = "Blue Wallet".to_string();
    let text_color_blue = "#1a578f".to_string();

    // renders the guides/basic/* route
    view! {
        <div id="basic" class="grid gap-6 max-w-3xl mx-auto mt-8 mb-24 animate-fadeinone grid-rows-[auto_auto_1fr] lg:max-w-4xl xl:max-w-5xl lg:gap-8">
            // Section 1: Title, Quote, and Quote Author
            <div class="mt-10 lg:mt-0 px-6">
                <h1 class="text-center text-[2.25rem] text-[#f7931a] font-semibold md:text-[2.5rem] lg:text-[3rem]">{title}</h1>
                <div class="text-center max-w-sm mx-auto">
                    <p class="text-lg text-white italic">{quote}</p>
                </div>
                <div class="text-center max-w-sm mx-auto">
                    <p class="text-sm text-white italic">{quote_author}</p>
                </div>
            </div>

            // Section 2: Intro
            <div class="px-6 pt-4 lg:pt-0 lg:px-0">
                <p class="text-xl font-semibold text-white pb-2 ">"Bitcoin Self-Custody: The act of taking possession of a bitcoin private key."</p>
                <p class="text-lg text-white pb-2">{intro}</p>
                <p class="text-lg text-white pb-2"><strong>{wallet_name_blue.clone()}</strong>{wallet_one_text}</p>
                <p class="text-lg text-white"><strong>{wallet_name_mutiny.clone()}</strong>{wallet_two_text}</p>
            </div>

            // Section 3: Everything Else
            <div class="px-6 lg:pb-4 lg:px-0">
                <hr class="border border-solid border-gray-400 mx-auto w-full mb-6"/>

                <h2 class="text-center pb-4 text-[1.5rem] font-semibold text-[#f7931a] font-semibold">"Pick A Wallet"</h2>

                <div class="flex flex-col mx-auto justify-center lg:flex-row px-6 gap-4">
                    <WalletButton on_click=move |_| {set_blue_clicked(true); set_blue_details(true);}
                        selected_wallet=WalletName::Blue platform=platform()
                        wallet_title=wallet_name_blue.clone() short_desc=short_desc_blue.clone() img_url=img_url_blue.clone()
                        img_alt=img_alt_blue.clone() text_color=text_color_blue.clone() />
                    <WalletButton on_click=move |_| {set_mutiny_clicked(true); set_mutiny_details(true);}
                        selected_wallet=WalletName::Mutiny platform=platform()
                        wallet_title=wallet_name_mutiny.clone() short_desc=short_desc_mutiny.clone() img_url=img_url_mutiny.clone() img_alt=img_alt_mutiny.clone()
                        text_color=text_color_mutiny.clone() />
                </div>
            </div>
        </div>
    }
}

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
        <div id="basic" class="grid gap-6 mx-auto max-w-3xl mt-8 mb-24 rounded-xl animate-fadeinone grid-rows-[auto_auto_1fr] lg:max-w-4xl xl:max-w-5xl lg:gap-8">
            // Section 1: Title, Quote, and Quote Author
            <div class="mt-10 lg:mt-0">
                <h1 class="text-center text-[2.25rem] px-6 text-[#f7931a] font-semibold md:text-[2.5rem] lg:text-[3rem] lg:px-0">{title}</h1>
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

                <h2 class="text-center pb-4 text-xl font-semibold text-[#f7931a]">"Recommended Wallet"</h2>

                <div class="flex flex-col justify-center lg:flex-row px-6 py-2 gap-4 max-w-2xl mx-auto">
                    <WalletButton on_click=move |_| {set_sparrow_clicked(true);}
                        selected_wallet=WalletName::Sparrow platform="desktop".to_string()
                        wallet_title=wallet_name_sparrow.clone() short_desc=short_desc_sparrow.clone() img_url=img_url_sparrow.clone()
                        img_alt=img_alt_sparrow.clone() text_color=text_color_sparrow.clone() />
                </div>
            </div>
        </div>

    }
}

// Renders the basic Android page
// This comp should be reviewed and likely redundant.
#[component]
#[allow(non_snake_case)]
pub fn RenderAndroidPage() -> impl IntoView {
    let intro_text: String = "This basic Android setup is meant to get you up to speed quickly. You'll pick one of the one wallets
        below, create your private key and take possession of your bitcoin. I wouldn't recommend storing too much of your wealth in a
        mobile wallet. Think of it as a self-custodied spending wallet, similar to how you'd carry cash in a physical wallet.
        ".to_string();

    let wallet_one_text: String = " is a great self-custodial On-chain wallet. It's easy to setup, follows all the latest standards and 
        also has the option of connecting to your own Lightning Node.".to_string();

    let wallet_two_text: String = " is a relatively new, modern On-chain and Lightning enabled wallet, it integrates bitcoin payments
        into your social network using the power of the decentralized NOSTR protocol and simplifies onboarding by making use of Fedimints. 
        I recommend Mutiny if you want more than just a basic bitcoin wallet but if you prefer to keep it simple
        choose Blue Wallet.".to_string();

    let title = "Basic Android Self-Custody Guide".to_string();
    let quote = "Trusted Third Parties are Security Holes".to_string();
    let quote_author = "-Nick Szabo".to_string();

    view! {
        <BeginnerPageTemplate title=title quote=quote quote_author=quote_author intro=intro_text wallet_one_text=wallet_one_text wallet_two_text=wallet_two_text/>
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

    let wallet_one_text: String = " is a great self-custodial On-Chain wallet. It's easy to setup, follows all the latest standards and 
        also has the option of connecting to your own Lightning Node.".to_string();

    let wallet_two_text: String = " is a relatively new, modern On-chain and Lightning enabled wallet, it integrates bitcoin payments
        into your social network using the power of the decentralized NOSTR protocol and simplifies onboarding by making use of Fedimints. 
        I recommend Mutiny if you want more than just a basic bitcoin wallet but if you prefer to keep it simple
        choose Blue Wallet.".to_string();

    let title = "Basic iOS Self-Custody Guide".to_string();
    let quote = "Trusted Third Parties are Security Holes".to_string();
    let quote_author = "-Nick Szabo".to_string();

    view! {
            <BeginnerPageTemplate title=title quote=quote quote_author=quote_author intro=intro_text wallet_one_text=wallet_one_text wallet_two_text=wallet_two_text/>

    }
}

/// Renders the basic IOS page.
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
            <BeginnerDesktopPageTemplate title=title quote=quote quote_author=quote_author intro=intro_text/>

    }
}
/// Route for the android instructions - renders either bluewallet or mutinywallet
/// depends on button clicked.
#[component]
#[allow(non_snake_case)]
pub fn BeginnerWalletInstructions(
    selected_wallet: WalletName,
    ios: bool,
) -> impl IntoView {
    let google_play_logo = "./../../../google-play-logo.avif".to_string();
    let google_play_alt = "Google Play Logo".to_string();

    let apple_store_logo = "./../../../download_on_app_store.svg".to_string();
    let apple_store_alt = "Apple Store Logo".to_string();

    let img_url_github = "./../../../github-mark.png".to_string();
    let img_alt_github = "Github Logo".to_string();

    let blue_google_play =
        r"https://play.google.com/store/apps/details?id=io.bluewallet.bluewallet".to_string();
    let blue_apple_store =
        r"https://apps.apple.com/app/bluewallet-bitcoin-wallet/id1376878040"
            .to_string();
    let blue_android_apk =
        r"https://github.com/BlueWallet/BlueWallet/releases".to_string();
    let mutiny_google_play =
        r"https://play.google.com/store/apps/details?id=com.mutinywallet.mutinywallet"
            .to_string();
    let mutiny_android_apk =
        r"https://github.com/MutinyWallet/mutiny-web/releases".to_string();

    let mutiny_apple_store =
        r"https://apps.apple.com/us/app/mutiny-wallet/id6471030760?ign-itscg=30200&ign-itsct=apps_box_link"
            .to_string();

    let sparrow_download = r"https://sparrowwallet.com/download/".to_string();
    let img_url_sparrow = "./../../../sparrow.png".to_string();
    let img_alt_sparrow = "Sparrow logo".to_string();

    let (displayed_wallet, set_displayed_wallet) = create_signal("");

    match selected_wallet {
        WalletName::Mutiny => set_displayed_wallet("mutiny"),
        WalletName::Blue => set_displayed_wallet("blue"),
        WalletName::Sparrow => set_displayed_wallet("sparrow"),
    }

    if displayed_wallet() == "blue" {
        view! {
            <div id="basic" class="grid gap-6 max-w-3xl mx-auto mt-8 mb-24 animate-fadeinone grid-rows-[auto_auto_1fr] lg:max-w-4xl xl:max-w-5xl lg:gap-8">
                // Section 1: Title, Quote, and Quote Author
                <div class="lg:mt-0 px-6">
                    <h1 class="text-center text-[2.25rem] font-semibold text-[#f7931a] md:text-[2.5rem] lg:text-[3rem]">"Blue Wallet"</h1>
                    <div class="text-center mx-auto">
                        <p class="text-lg font-semibold text-white italic">"Radically Simple 👩‍🎤 Extremely Powerful."</p>
                    </div>
                    <div class="text-center mx-auto">
                        <p class="text-md text-white italic">"A freedom and self-sovereign tool, disguised as a cute little Blue app in your pocket."</p>
                    </div>
                </div>

                // Section 2: Download Options
                <div class="flex flex-col mx-auto justify-center px-6 py-2 max-w-2xl mx-auto gap-4">
                    <Show
                        when=move || ios
                        fallback=move || view! {
                            <DownloadButton href=blue_google_play.clone() logo=google_play_logo.clone() alt_txt=google_play_alt.clone() button_name="Google Play".to_string()/>
                            <DownloadButton href=blue_android_apk.clone() logo=img_url_github.clone() alt_txt=img_alt_github.clone() button_name="APK".to_string()/>
                        }>
                        <DownloadButton href=blue_apple_store.clone() logo=apple_store_logo.clone() alt_txt=apple_store_alt.clone()/>
                    </Show>
                </div>

                // Section 3: Start Here
                <div class="mx-auto max-w-5xl p-4 w-full">
                    <div class="mx-auto border border-solid border-gray-400"></div>
                    <h2 class="flex justify-center font-semibold text-[#f7931a] text-[1.5rem] pt-6 pb-4">"Start Here"</h2>
                    <AccordionMenu faq_name="bluewallet".to_string()/>
                </div>
            </div>
        }
    } else if displayed_wallet() == "mutiny" {
        view! {
            <div id="basic" class="grid gap-6 max-w-3xl mx-auto mt-8 mb-24 animate-fadeinone grid-rows-[auto_auto_1fr] lg:max-w-4xl xl:max-w-5xl lg:gap-8">
                // Section 1: Title, Quote, and Quote Author
                <div class="lg:mt-0 px-6">
                    <h1 class="text-center text-[2.25rem] font-semibold text-[#f7931a] md:text-[2.5rem] lg:text-[3rem]">"Mutiny Wallet"</h1>
                    <div class="text-center mx-auto">
                        <p class="text-lg font-semibold text-white italic">"Unstoppable bitcoin. For everyone."</p>
                    </div>
                    <div class="text-center mx-auto">
                        <p class="text-md text-white italic">"Mutiny is a self-custodial lightning wallet that runs everywhere."</p>
                    </div>
                </div>

                // Section 2: Download Options
                <div class="flex flex-col mx-auto justify-center px-6 py-2 max-w-2xl mx-auto gap-4">
                    <Show
                        when=move || ios
                        fallback=move || view! {
                            <DownloadButton href=mutiny_google_play.clone() logo=google_play_logo.clone() alt_txt=google_play_alt.clone() button_name="Google Play".to_string()/>
                            <DownloadButton href=mutiny_android_apk.clone() logo=img_url_github.clone() alt_txt=img_alt_github.clone() button_name="APK".to_string()/>
                        }>
                        <DownloadButton href=mutiny_apple_store.clone() logo=apple_store_logo.clone() alt_txt=apple_store_alt.clone()/>
                    </Show>
                </div>

                // Section 3: Start Here
                <div class="mx-auto max-w-5xl p-4 w-full">
                    <div class="mx-auto border border-solid border-gray-400"></div>
                    <h2 class="flex justify-center font-semibold text-[#f7931a] text-[1.5rem] pt-6 pb-4">"Start Here"</h2>
                    <AccordionMenu faq_name="mutiny".to_string()/>
                </div>
            </div>
        }
    } else {
        view! {
            <div id="basic" class="grid gap-6 max-w-3xl mx-auto mt-8 mb-24 animate-fadeinone grid-rows-[auto_auto_1fr] lg:max-w-4xl xl:max-w-5xl lg:gap-8">
                // Section 1: Title, Quote, and Quote Author
                <div class="lg:mt-0 px-6">
                    <h1 class="text-center text-[2.25rem] font-semibold text-[#f7931a] md:text-[2.5rem] lg:text-[3rem]">"Sparrow Wallet"</h1>
                    <div class="text-center mx-auto">
                        <p class="text-white text-lg font-semibold px-4">"Gain Financial Sovereignty with Sparrow Wallet."</p>
                    </div>
                </div>

                // Section 2: Download Options
                <div class="flex flex-col mx-auto justify-center px-6 py-2 mx-auto gap-4 lg:px-4">
                    <DownloadButton href=sparrow_download.clone() logo=img_url_sparrow.clone() alt_txt=img_alt_sparrow.clone() button_name="Get Sparrow".to_string()/>
                </div>

                // Section 3: Start Here
                <div class="mx-auto max-w-5xl p-4 w-full">
                    <div class="mx-auto border border-solid border-gray-400"></div>
                    <h2 class="flex justify-center font-semibold text-[#f7931a] text-[1.5rem] pt-6 pb-4">"Start Here"</h2>
                    <AccordionMenu faq_name="sparrow".to_string()/>
                </div>
            </div>
        }
    }
}
