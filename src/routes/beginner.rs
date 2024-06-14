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
            <button class="flex justify-center shrink-0 h-20 w-72 p-4 mx-auto bg-white rounded-xl items-center space-x-4 shadow-inner" on:click=on_click>
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
    let (width, set_width) = create_signal(8);
    let (height, set_heigth) = create_signal(8);
    let (flex_justify, set_flex_justify) = create_signal(String::new());

    let name = match button_name.clone() {
        Some(name) => name,
        None => "No Button Name".to_string(),
    };

    set_button(name.clone());

    if button_name.is_none() {
        set_width(36);
        set_heigth(10);
        set_flex_justify("justify-center".to_string());
    }
    view! {
        <a href=href target="_blank" rel="external">
            <button class=format!("flex {} p-2 shrink-0 h-12 w-36 mx-auto bg-white rounded-xl items-center space-x-4 shadow-inner", flex_justify.get_untracked())>
                <div class="shrink">
                    <img class=format!("h-{} w-{}", height.get(), width.get()) src=format!("{}", logo) alt=format!("{}", alt_txt) />
                </div>
                <Show
                    when=move || button_name.is_some()
                    fallback=move || view!("")>
                    <div class="">
                        <p class="font-semibold text-sm">
                            {button().to_string()}
                        </p>
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
    #[prop(optional)] intro_part_two: String,
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

    // DOM elements are only available when used within a create_effect see --> https://leptos-rs.github.io/leptos/ssr/24_hydration_bugs.html#not-all-client-code-can-run-on-the-server
    // create_effect(move |_| {
    //     let beginner_guide_element = window().document().unwrap().get_element_by_id("beginner");
    //     log!("guide element: {:?}", beginner_guide_element);
    // });

    //window_event_listener(ev::animationend, move |_e| {
    //        set_slideout_ends(true);
    //        log!("Animation Done");
    // });

    // renders the guides/basic/* route
    view! {
        <div id="basic" class="flex flex-col max-w-3xl mx-auto pb-10 animate-fadeinone md:transform md:scale-125 md:pt-20" >
            <div class="flex flex-col p-6 pt-10 max-w-3xl mx-auto">
                    <h1 class="flex justify-center text-center text-[36px] text-white font-semibold">{title}</h1>
                <div class="flex justify-start pt-4 max-w-sm">
                    <p class="text-lg text-white italic">{quote}</p>
                </div>
                <div class="flex max-w-sm">
                    <p class="text-sm text-white italic">{quote_author}</p>
                </div>
            </div>

            <div class="flex flex-col p-6 max-w-3xl mx-auto" >
                <p class="font-bold text-white">"Bitcoin Self-Custody:"</p>
                <p class="pb-2 text-white">"The act of taking possession of a bitcoin private key."</p>
                <p class="mr-4 text-md text-white">{intro}</p>
                <br></br>
                <p class="mr-4 text-md text-white">{intro_part_two}</p>
            </div>

            <div class="mx-auto max-w-xl p-4 w-full" >
                <div class="mx-auto border border-solid border-gray-400"></div>
            </div>

            <div class="flex flex-col mx-auto justify-center" >
                <h2 class="flex justify-center pb-4 max-w-2xl text-center mx-auto text-xl text-white" >"Pick A Wallet"</h2>
            </div>
            <div class="flex flex-col md:flex-row px-6 py-2 max-w-2xl mx-auto gap-4">
                <WalletButton on_click = move |_| {set_blue_clicked(true); set_blue_details(true);}
                        selected_wallet=WalletName::Blue platform=platform()
                        wallet_title=wallet_name_blue.clone() short_desc=short_desc_blue.clone() img_url=img_url_blue.clone()
                        img_alt=img_alt_blue.clone() text_color=text_color_blue.clone()
                    />
                <WalletButton on_click = move |_| {set_mutiny_clicked(true); set_mutiny_details(true);}
                        selected_wallet=WalletName::Mutiny platform=platform()
                        wallet_title=wallet_name_mutiny.clone() short_desc=short_desc_mutiny.clone() img_url=img_url_mutiny.clone() img_alt=img_alt_mutiny.clone()
                        text_color=text_color_mutiny.clone()
                    />
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
        <div id="basic" class="flex flex-col max-w-3xl mx-auto rounded-xl pb-10 animate-fadeinone md:transform md:scale-125 md:pt-20" >
            <div class="flex flex-col p-6 pt-10 max-w-3xl mx-auto">
                    <h1 class="flex justify-center text-center text-[36px] text-white font-semibold">{title}</h1>
                <div class="flex justify-start pt-4 max-w-sm">
                    <p class="text-lg text-white italic">{quote}</p>
                </div>
                <div class="flex max-w-sm">
                    <p class="text-sm text-white italic">{quote_author}</p>
                </div>
            </div>

            <div class="flex flex-col p-6 max-w-3xl mx-auto" >
                //<p class="font-bold text-white">"Basic Desktop Wallet Setup:"</p>
                //<p class="pb-2 text-white">"Sparrow Wallet"</p>
                <p class="mr-4 text-md text-white">{intro}</p>
            </div>

            <div class="mx-auto max-w-xl p-4 w-full" >
                <div class="mx-auto border border-solid border-gray-400"></div>
            </div>

            <div class="flex flex-col mx-auto justify-center" >
                <h2 class="flex justify-center pb-4 max-w-2xl text-center mx-auto text-xl text-white" >"Recommended Wallet"</h2>
            </div>
            <div class="flex flex-col md:flex-row px-6 py-2 max-w-2xl mx-auto gap-4">

                <WalletButton on_click = move |_| {set_sparrow_clicked(true);}
                    selected_wallet=WalletName::Sparrow platform="desktop".to_string()
                    wallet_title=wallet_name_sparrow.clone() short_desc=short_desc_sparrow.clone() img_url=img_url_sparrow.clone()
                    img_alt=img_alt_sparrow.clone() text_color=text_color_sparrow.clone()
                    />

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
        below, create your private key and take posession of your bitcoin. I wouldn't recommend storing too much of your wealth in a
        mobile wallet. Think of it as a self-custodied spending wallet, similar to how you'd carry cash in a physical wallet.
        ".to_string();

    let intro_part_two: String = "Blue Wallet is a great self-custodial On-Chain wallet. It's easy to setup, follows all the latest standards and 
        also has the option of connecting to your own Lightning Node. Mutiny is a relative new, modern On-chain and Lightning enabled wallet, it integrats bitcoin payments
        into your social networks using the power of the decentralized NOSTR protocol. I recommend Mutiny if you want more the just a basic bitcoin wallet but if you prefer to keep it simple
        choose Blue Wallet.".to_string();

    let title = "Android Self-Custody Guide".to_string();
    let quote = "Trusted Third Parties are Security Holes".to_string();
    let quote_author = "-Nick Szabo".to_string();

    view! {
        <BeginnerPageTemplate title=title quote=quote quote_author=quote_author intro=intro_text intro_part_two=intro_part_two/>
    }
}

/// Renders the basic IOS page.
#[component]
#[allow(non_snake_case)]
pub fn RenderIosPage() -> impl IntoView {
    let intro_text: String = "This basic IOS setup is meant to get you up to speed quickly. You'll pick one of the one wallets
        below, create your private key and take posession of your bitcoin. I wouldn't recommend storing too much of your wealth in a
        mobile wallet. Think of it as a self-custodied spending wallet, similar to how you'd carry cash in a physical wallet.
        ".to_string();

    let intro_part_two: String = "Blue Wallet is a great self-custodial On-Chain wallet. It's easy to setup, follows all the latest standards and 
        also has the option of connecting to your own Lightning Node. Mutiny is a relative new, modern On-chain and Lightning enabled wallet, it integrates bitcoin payments
        into your social networks using the power of the decentralized NOSTR protocol. I recommend Mutiny if you want more the just a basic bitcoin wallet but if you prefer to keep it simple
        choose Blue Wallet.".to_string();

    let title = "Beginner - IOS Self-Custody Guide".to_string();
    let quote = "Trusted Third Parties are Security Holes".to_string();
    let quote_author = "-Nick Szabo".to_string();

    view! {
            <BeginnerPageTemplate title=title quote=quote quote_author=quote_author intro=intro_text intro_part_two=intro_part_two/>

    }
}

/// Renders the basic IOS page.
#[component]
#[allow(non_snake_case)]
pub fn RenderDesktopPage() -> impl IntoView {
    let intro_text: String = "Desktop wallets, such as Sparrow Wallet, deliver heightened security versus mobile options. 
        Often employed in elaborate setups for self-managing sizeable Bitcoin savings, they remain accessible even for basic use cases. 
        Our guide begins with a simplified configuration, expanding upon it later. Ideal for individuals intending to grow their Bitcoin holdings, 
        this introduction sets the stage for more advanced techniques.".to_string();

    let title = "Beginner - Desktop Self-Custody Guide".to_string();
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
        // Render Blue Wallet instructions
        view! {
            <div class="flex flex-col max-w-3xl p-4 pt-8 mx-auto rounded-xl animate-fadeinone md:transform md:scale-125 md:pt-28">
                <h1 class="flex justify-center text-[36px] font-bold text-[#83d1f4]">"Blue Wallet"</h1>
                <div class="flex flex-col text-center">
                    <p class="text-white px-4">
                        "Radically Simple 👩‍🎤 Extremely Powerful."
                    </p>
                    <p class="text-white text-sm">
                        "A freedom and self-sovereign tool, disguised as a cute little Blue app in your pocket."
                    </p>
                </div>
                <br></br>
                <h2 class="flex justify-center font-bold text-xl text-white py-2">"Download Options"</h2>

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

                <div class="mx-auto max-w-xl p-4 w-full" >
                    <div class="mx-auto border border-solid border-gray-400"></div>
                </div>

                <h2 class="flex justify-center font-bold text-xl text-white pt-6 pb-2">"Start Here"</h2>
                <AccordionMenu faq_name="bluewallet".to_string()/>
            </div>
        }
    } else if displayed_wallet() == "mutiny" {
        // Render Samourai wallet instructions
        view! {
            <div class="flex flex-col max-w-3xl p-4 pt-8 mx-auto rounded-xl animate-fadeinone md:transform md:scale-125 md:pt-28">
                <div class="flex flew-row justify-center">
                    <h1 class="flex justify-center text-[36px] font-bold text-[#f71d5a]">"Mutiny Wallet"</h1>
                </div>
                <div class="flex flex-col text-center">
                    <p class="text-white text-center px-4">
                        "Unstoppable bitcoin. For everyone."
                    </p>
                    <p class="text-white text-center text-sm">
                        "Mutiny is a self-custodial lightning wallet that runs everywhere."
                    </p>
                </div>
                <br></br>
                <h2 class="flex justify-center font-bold text-xl text-white py-2">"Download Options"</h2>
                <div class="flex flex-col justify-center px-6 py-2 max-w-2xl mx-auto space-y-4">
                    <Show
                        when=move || ios
                        fallback=move || view! {

                        <DownloadButton href=mutiny_google_play.clone() logo=google_play_logo.clone() alt_txt=google_play_alt.clone() button_name="Google Play".to_string()/>
                        <DownloadButton href=mutiny_android_apk.clone() logo=img_url_github.clone() alt_txt=img_alt_github.clone() button_name="APK".to_string()/>
                            }>
                        <DownloadButton href=mutiny_apple_store.clone() logo=apple_store_logo.clone() alt_txt=apple_store_alt.clone()/>
                    </Show>
                </div>

                <div class="mx-auto max-w-xl p-4 w-full" >
                    <div class="mx-auto border border-solid border-gray-400"></div>
                </div>

                <h2 class="flex justify-center font-bold text-xl text-white pt-6 pb-2">"Start Here"</h2>
                <AccordionMenu faq_name="mutiny".to_string()/>
            </div>
        }
    } else {
        // Render Sparrow wallet instructions
        view! {
        <div class="flex flex-col max-w-3xl p-4 pt-8 mx-auto animate-fadeinone md:transform md:scale-125 md:pt-28">
            <div class="flex flew-row justify-center">
                <h1 class="flex justify-center text-[36px] font-bold text-[#BEAE9A]">"Sparrow Wallet"</h1>
            </div>
            <div class="flex flex-col items-center">
                <p class="text-white text-center px-4 text-sm">
                    "Gain Financial Sovereignty with Sparrow Wallet."
                </p>
            </div>
            <br></br>
            <h2 class="flex justify-center font-bold text-xl text-white py-2">"Download Options"</h2>
            <div class="flex flex-col justify-center px-6 py-2 max-w-2xl mx-auto space-y-4">
                <DownloadButton href=sparrow_download.clone() logo=img_url_sparrow.clone() alt_txt=img_alt_sparrow.clone() button_name="Get Sparrow".to_string()/>
            </div>

            <div class="mx-auto max-w-xl p-4 w-full" >
                <div class="mx-auto border border-solid border-gray-400"></div>
            </div>

            <h2 class="flex justify-center font-bold text-xl text-white pt-6 pb-2">"Start Here"</h2>
            <AccordionMenu faq_name="sparrow".to_string()/>
        </div>
        }
    }
}
