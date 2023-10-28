use leptos::{ev::MouseEvent, *};

use crate::extras::accordion_menu::AccordionMenu;
use crate::helpers::get_path::get_current_path;

#[component]
#[allow(non_snake_case)]
pub fn WalletButton<F>(
    on_click: F,
    wallet_name: String,
    short_desc: String,
    img_url: String,
    img_alt: String,
    text_color: String,
    samourai: bool,
    blue: bool,
    _green: bool,
    platform: String,
) -> impl IntoView
where
    F: Fn(MouseEvent) + 'static,
{
    // determine which wallet button was clicked on
    let (wallet, set_wallet) = create_signal("".to_string());
    if samourai {
        set_wallet("samourai".to_string())
    } else if blue {
        set_wallet("blue".to_string())
    } else {
        set_wallet("blockstream".to_string())
    }

    // get the name of the wallet
    let wallet = wallet();
    // create our url path
    let path = format!("/guides/beginner/{platform}/{wallet}");

    view! {
        <a href=path>
            <button class="flex justify-center shrink-0 h-20 w-72 p-4 mx-auto bg-white rounded-xl items-center space-x-4" on:click=on_click>
              <div class="shrink-0">
                <img class="h-12 w-12" src=img_url alt=img_alt/>
              </div>
              <div>
                <h3 class=format!("text-xl font-medium text-[{text_color}]")>{wallet_name}</h3>
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
    let (button, set_button) = create_signal("".to_string());
    let (width, set_width) = create_signal(6);
    let (height, set_heigth) = create_signal(6);
    let (flex_justify, set_flex_justify) = create_signal("".to_string());

    let name = match button_name.clone() {
        Some(name) => name,
        None => "No Button Name".to_string(),
    };

    set_button(name.clone());

    if button_name.is_none() {
        set_width(36);
        set_heigth(10);
        set_flex_justify("justify-center".to_string())
    }

    view! {
        <a href=href target="_blank" rel="external">
            <button class=format!("flex {} p-2 shrink-0 h-12 w-36 mx-auto bg-white rounded-xl items-center space-x-4", flex_justify())>
                <div class="shrink-0">
                    <img class=format!("h-{} w-{}", height(), width()) src=format!("{}", logo) alt=format!("{}", alt_txt) />
                </div>
                <Show
                    when=move || button_name.is_some()
                    fallback=move || view!("")>
                    <p class="font-medium text-sm">
                        {format!("{}", button())}
                    </p>
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
) -> impl IntoView {
    // used for onlick to determine which button was clicked
    let (_samourai_clicked, _set_samourai_clicked) = create_signal(false);
    let (_samourai_clicked, _set_samourai_clicked) = create_signal(false);
    let (_blue_clicked, set_blue_clicked) = create_signal(false);
    let (_green_clicked, _set_green_clicked) = create_signal(false);

    // set the button details
    let (_samourai_details, set_samourai_details) = create_signal(false);
    let (_blue_details, set_blue_details) = create_signal(false);
    let (_green_details, _set_green_details) = create_signal(false);

    // get current path via RouteContext
    let path = get_current_path();
    let (platform, set_platform) = create_signal("".to_string());

    if path.contains("ios") {
        set_platform("ios".to_string())
    } else {
        set_platform("android".to_string())
    }

    // Samourai wallet assets
    let wallet_name_samourai = "Samourai Wallet".to_string();
    let short_desc_samourai = "Privacy + Freedom Tools".to_string();
    let img_url_samourai = "./../../samourai_logo.png".to_string();
    let img_alt_samourai = "Samourai Logo".to_string();
    let text_color_samourai = "#1a578f".to_string(); // actual colour should be: #c0272b

    // Blue wallet assets
    let wallet_name_blue = "Blue Wallet".to_string();
    let short_desc_blue = "Basic + Ease of Use".to_string();
    let img_url_blue = "./../../bluewallet_logo.webp".to_string();
    let img_alt_blue = "Blue Wallet".to_string();
    let text_color_blue = "#1a578f".to_string();

    // BlockStream Green wallet assets
    let _wallet_name_green = "BlockStream Green".to_string();
    let _short_desc_green = "Self-Custody Made Easy".to_string();
    let _img_url_green = "./../../nav_green.svg".to_string();
    let _img_alt_green = "BlockStream Green".to_string();
    let _text_color_green = "#0a7b46ff".to_string();

    // DOM elements are only available when used within a create_effect see --> https://leptos-rs.github.io/leptos/ssr/24_hydration_bugs.html#not-all-client-code-can-run-on-the-server
    // create_effect(move |_| {
    //     let beginner_guide_element = window().document().unwrap().get_element_by_id("beginner");
    //     log!("guide element: {:?}", beginner_guide_element);
    // });

    //window_event_listener(ev::animationend, move |_e| {
    //        set_slideout_ends(true);
    //        log!("Animation Done");
    // });

    // renders the guides/beginner/* route
    view! {
        <div id="beginner" class="flex flex-col max-w-3xl mx-auto rounded-xl pb-10 animate-fadein" >
            <div class="flex flex-col p-6 pt-10 max-w-3xl mx-auto">
                    <h1 class="flex justify-center text-[36px] text-white font-semibold">{title}</h1>
                <div class="flex justify-start pt-4 max-w-sm">
                    <p class="text-lg text-white italic">{quote}</p>
                </div>
                <div class="flex max-w-sm">
                    <p class="text-sm text-white italic">{quote_author}</p>
                </div>
            </div>

            <div class="flex flex-col p-6 max-w-2xl mx-auto bg-[#1a578f] rounded-xl shadow-xl" >
                <p class="font-bold text-white">"Bitcoin Self-Custody:"</p>
                <p class="pb-2 text-white">"The act of taking possession of a bitcoin private key."</p>
                <p class="mr-4 text-lg text-white">{intro}</p>
            </div>

            <div class="mx-auto max-w-xl p-4 w-full" >
                <div class="mx-auto border border-solid border-gray-400"></div>
            </div>

            <div class="flex flex-col mx-auto justify-center" >
                <h2 class="flex justify-center pb-4 max-w-2xl text-center mx-auto text-xl text-white" >"Pick A Wallet"</h2>
            </div>
            <div class="flex flex-col md:flex-row px-6 py-2 max-w-2xl mx-auto gap-4">
                // default is geen wallet - display samourai wallet if android guide was selected.
                // Blue wallet is available for both android/ios
                <Show
                   when=move || platform() == "android".to_string()
                   fallback= move || view! {
                      // comment out greenWallet for now leaving 1 wallet option for IOS
                      // { <WalletButton on_click = move |_| {set_green_clicked(true);
                      //           set_green_details(true)}
                      //          samourai=false blue=false _green=true platform=platform()
                      //          wallet_name=wallet_name_green.clone() short_desc=short_desc_green.clone() img_url=img_url_green.clone() img_alt=img_alt_green.clone()
                      //          text_color=text_color_green.clone()
                      //          />}
                   }
                >
                        <WalletButton on_click = move |_| {_set_samourai_clicked(true);
                                set_samourai_details(true)}
                                samourai=true blue=false _green=false platform=platform()
                                wallet_name=wallet_name_samourai.clone() short_desc=short_desc_samourai.clone() img_url=img_url_samourai.clone() img_alt=img_alt_samourai.clone()
                                text_color=text_color_samourai.clone()
                                />
                </Show>

                <WalletButton on_click = move |_| {set_blue_clicked(true);
                        set_blue_details(true)}
                        blue=true samourai=false _green=false platform=platform()
                        wallet_name=wallet_name_blue.clone() short_desc=short_desc_blue.clone() img_url=img_url_blue.clone()
                        img_alt=img_alt_blue.clone() text_color=text_color_blue.clone()
                        />
            </div>
        </div>
    }
}

// Renders the beginner Android page
// This comp should be reviewed and likely redundant.
#[component]
#[allow(non_snake_case)]
pub fn BeginnerPageAndroid() -> impl IntoView {
    let intro_text: String = "Controlling a bitcoin private key grants absolute authority over the
        associated bitcoin, embodying the ethos of the bitcoin movement. Self custody and personal
        responsibility restore power and sovereignty, eliminating reliance on third parties,
        particularly the state."
        .to_string();

    let title = "Android Self-Custody Guide".to_string();
    let quote = "Trusted Third Parties are Security Holes".to_string();
    let quote_author = "-Nick Szabo".to_string();

    view! {
        <BeginnerPageTemplate title=title quote=quote quote_author=quote_author intro=intro_text/>
    }
}

/// Renders the beginner IOS page.
#[component]
#[allow(non_snake_case)]
pub fn BeginnerPageIOS() -> impl IntoView {
    let intro_text: String = "Controlling a bitcoin private key grants absolute authority over the
            associated bitcoin, embodying the ethos of the bitcoin movement. Self custody and personal
            responsibility restore power and sovereignty, eliminating reliance on third parties,
            particularly the state.".to_string();

    let title = "Beginner - IOS Self-Custody Guide".to_string();
    let quote = "Trusted Third Parties are Security Holes".to_string();
    let quote_author = "-Nick Szabo".to_string();

    view! {
            <BeginnerPageTemplate title=title quote=quote quote_author=quote_author intro=intro_text/>

    }
}

/// Route for the android instructions - renders either bluewallet or samouraiwallet
/// depends on button clicked.
#[component]
#[allow(non_snake_case)]
pub fn BeginnerWalletInstructions(
    blue: bool,
    samourai: bool,
    _green: bool,
    ios: bool,
) -> impl IntoView {
    let google_play_logo = "./../../../google-play-logo.avif".to_string();
    let google_play_alt = "Google Play Logo".to_string();

    let apple_store_logo = "./../../../download_on_app_store.svg".to_string();
    let apple_store_alt = "Apple Store Logo".to_string();

    let img_url_github = "./../../../github-mark.png".to_string();
    let img_alt_github = "Github Logo".to_string();

    let img_url_samourai_fdroid = "./../../../F-Droid_Logo_4.svg".to_string();
    let img_alt_samourai_fdroid = "F-Droid Logo".to_string();

    let blue_google_play =
        r"https://play.google.com/store/apps/details?id=io.bluewallet.bluewallet".to_string();
    let blue_apple_store =
        r"https://apps.apple.com/app/bluewallet-bitcoin-wallet/id1376878040".to_string();
    let blue_android_apk = r"https://github.com/BlueWallet/BlueWallet/releases".to_string();

    let samourai_google_play =
        r"https://play.google.com/store/apps/details?id=com.samourai.wallet&hl=en_US&gl=US"
            .to_string();
    let samourai_android_apk = r"https://samouraiwallet.com/download".to_string();
    let samourai_fdroid = r"https://samouraiwallet.com/download/fdroid".to_string();

    let green_apple_store =
        r"https://apps.apple.com/us/app/green-bitcoin-wallet/id1402243590".to_string();

    if blue {
        // Render Blue Wallet instructions
        view! {
            <div class="flex flex-col max-w-3xl p-4 pt-8 mx-auto rounded-xl animate-fadein">
                <h1 class="flex justify-center text-[36px] font-bold text-[#BEAE9A]">"Blue Wallet"</h1>
                <div class="flex flex-col items-center">
                    <p class="text-white items-center px-4">
                        "Radically Simple 👩‍🎤 Extremely Powerful."
                    </p>
                    <p class="text-white items-center text-sm">
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
                        <DownloadButton href=blue_android_apk.clone() logo=img_url_github.clone() alt_txt=img_alt_github.clone() button_name="Android APK".to_string()/>
                            }>
                        <DownloadButton href=blue_apple_store.clone() logo=apple_store_logo.clone() alt_txt=apple_store_alt.clone()/>
                    </Show>
                </div>

                <div class="mx-auto max-w-xl p-4 w-full" >
                    <div class="mx-auto border border-solid border-gray-400"></div>
                </div>

                <h2 class="flex justify-center font-bold text-xl text-white pt-6 pb-2">"Help Me!"</h2>
                <AccordionMenu faq_name="bluewallet".to_string()/>
            </div>
        }
    } else if samourai {
        // Render Samourai wallet instructions
        view! {
            <div class="flex flex-col max-w-3xl p-4 pt-8 mx-auto rounded-xl animate-fadein">
                <div class="flex flew-row justify-center">
                    <h1 class="flex justify-center text-[36px] font-bold text-[#BEAE9A]">"Samourai Wallet"</h1>
                </div>
                <div class="flex flex-col items-center">
                    <p class="text-white text-center px-4 text-sm">
                        "A modern bitcoin wallet hand forged to keep your transactions private your identity masked and your funds secured."
                    </p>
                </div>
                <br></br>
                <h2 class="flex justify-center font-bold text-xl text-white py-2">"Download Options"</h2>
                <div class="flex flex-col justify-center px-6 py-2 max-w-2xl mx-auto space-y-4">
                    <DownloadButton href=samourai_google_play.clone() logo=google_play_logo.clone() alt_txt=google_play_alt.clone() button_name="Google Play".to_string()/>
                    <DownloadButton href=samourai_fdroid.clone() logo=img_url_samourai_fdroid.clone() alt_txt=img_alt_samourai_fdroid.clone() button_name="F-Droid".to_string()/>
                    <DownloadButton href=samourai_android_apk.clone() logo=img_url_github.clone() alt_txt=img_alt_github.clone() button_name="Android APK".to_string()/>
                </div>

                <div class="mx-auto max-w-xl p-4 w-full" >
                    <div class="mx-auto border border-solid border-gray-400"></div>
                </div>

                <h2 class="flex justify-center font-bold text-xl text-white pt-6 pb-2">"Help Me!"</h2>
                <AccordionMenu faq_name="samourai".to_string()/>
            </div>
        }
    } else {
        // Render BlockStream wallet instructions
        view! {
        <div class="flex flex-col max-w-3xl p-4 pt-8 mx-auto rounded-xl animate-fadein">
            <div class="flex flew-row justify-center">
                <h1 class="flex justify-center text-[36px] font-bold text-[#BEAE9A]">"BlockStream Green Wallet"</h1>
            </div>
            <div class="flex flex-col items-center">
                <p class="text-white text-center px-4 text-sm">
                    "Blockstream Green is a simple and secure Bitcoin wallet that makes it easy to get started sending and receiving Bitcoin."
                </p>
            </div>
            <br></br>
            <h2 class="flex justify-center font-bold text-xl text-white py-2">"Download Options"</h2>
            <div class="flex flex-col justify-center px-6 py-2 max-w-2xl mx-auto space-y-4">
                <DownloadButton href=green_apple_store.clone() logo=apple_store_logo.clone() alt_txt=apple_store_alt.clone()/>
            </div>

            <div class="mx-auto max-w-xl p-4 w-full" >
                <div class="mx-auto border border-solid border-gray-400"></div>
            </div>

            <h2 class="flex justify-center font-bold text-xl text-white pt-6 pb-2">"Help Me!"</h2>
            <AccordionMenu faq_name="greenwallet".to_string()/>
        </div>
        }
    }
}
