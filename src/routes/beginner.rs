use leptos::{*, ev::MouseEvent};
use leptos_router::*;

use crate::extras::accordion_menu::AccordionMenu;
use crate::server::api::fetch_faqs::FAQ;


// get the current path via the RouteContext
fn get_current_path(cx: Scope) -> String {
    // Retrieve the URL path of the current route
    let current_page = use_route(cx).path();

    current_page 
}




#[component]
#[allow(non_snake_case)]
pub fn WalletButton<F>(cx: Scope, on_click: F, wallet_name: String, short_desc: String, img_url:
                        String, img_alt: String, text_color: String, samourai: bool, blue: bool, green: bool, platform: String) -> impl IntoView
    where
        F: Fn(MouseEvent) + 'static,
    {

    // determine which wallet button was clicked on 
    let (wallet, set_wallet) = create_signal(cx, "".to_string());
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
    
    view! {cx, 
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
pub fn DownloadButton(cx: Scope, href: String, logo: String, alt_txt: String, #[prop(optional)] button_name: Option<String> 
                        ) -> impl IntoView
    {

    let (button, set_button) = create_signal(cx, "".to_string());
    let (width, set_width) = create_signal(cx, 12);
    
    let name = match button_name.clone() {
        Some(name) => name,
        None => "No Button Name".to_string(),
    };

    set_button(name.clone());
    
    if button_name.is_none() {
        set_width(36)
    }
    
    view! {cx, 
        <a href=href target="_blank" rel="external">
            <button class="flex justify-center p-2 shrink-0 h-18 w-60 mx-auto bg-white rounded-xl items-center space-x-4">
                <div class="shrink-0">
                    <img class=format!("h-12 w-{}", width()) src=format!("{}", logo) alt=format!("{}", alt_txt) />
                </div>
                <Show
                    when=move || button_name.is_some()
                    fallback=move |_| view!(cx, "")> 
                    <div class="font-bold">
                        {format!("{}", button())}
                    </div>
                </Show>
            </button>
        </a>
    }
}       

#[component]
#[allow(non_snake_case)]
pub fn BeginnerPageTemplate(cx: Scope, title: String, quote: String, quote_author: String, intro:
                            String 
                            ) -> impl IntoView {

    // used for onlick to determine which button was clicked
    let (samourai_clicked, set_samourai_clicked) = create_signal(cx, false);
    let (blue_clicked, set_blue_clicked) = create_signal(cx, false);
    let (green_clicked, set_green_clicked) = create_signal(cx, false);

    // set the button details 
    let (samourai_details, set_samourai_details) = create_signal(cx, false);
    let (blue_details, set_blue_details) = create_signal(cx, false);
    let (green_details, set_green_details) = create_signal(cx, false);

    // get current path via RouteContext
    let path = get_current_path(cx);
    let (platform, set_platform) = create_signal(cx, "".to_string());

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
    let wallet_name_green = "BlockStream Green".to_string();
    let short_desc_green = "Self-Custody Made Easy".to_string();
    let img_url_green = "./../../nav_green.svg".to_string();
    let img_alt_green = "BlockStream Green".to_string();
    let text_color_green = "#0a7b46ff".to_string();

    // DOM elements are only available when used within a create_effect see --> https://leptos-rs.github.io/leptos/ssr/24_hydration_bugs.html#not-all-client-code-can-run-on-the-server
    create_effect(cx, move |_| {
        let beginner_guide_element = window().document().unwrap().get_element_by_id("beginner");
        log!("guide element: {:?}", beginner_guide_element);
    });

    //window_event_listener(ev::animationend, move |_e| {
    //        set_slideout_ends(true);
    //        log!("Animation Done");
    // });   

    // renders the guides/beginner/* route 
    view! { cx, 
        <div id="beginner" class="flex flex-col max-w-3xl mx-auto shadow-xl rounded-xl pb-10 animate-fadein" >
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
                <div class="mx-auto border border-dashed border-gray-400"></div>
            </div>

            <div class="flex flex-col mx-auto justify-center" >
                <h2 class="flex justify-center pb-4 max-w-2xl text-center mx-auto text-xl text-white" >"Pick A Wallet"</h2>
            </div>
            <div class="flex flex-col md:flex-row px-6 py-2 max-w-2xl mx-auto gap-4">
                // default is geen wallet - display samourai wallet if android guide was selected.
                // Blue wallet is available for both android/ios
                <Show
                   when=move || platform() == "android".to_string()
                   fallback= move |cx| view! {cx, 
                        <WalletButton on_click = move |_| {set_green_clicked(true);
                                 set_green_details(true)}
                                samourai=false blue=false green=true platform=platform()
                                wallet_name=wallet_name_green.clone() short_desc=short_desc_green.clone() img_url=img_url_green.clone() img_alt=img_alt_green.clone()
                                text_color=text_color_green.clone()
                                />
                   }
                > 
                        <WalletButton on_click = move |_| {set_samourai_clicked(true);
                                set_samourai_details(true)}
                                samourai=true blue=false green=false platform=platform()
                                wallet_name=wallet_name_samourai.clone() short_desc=short_desc_samourai.clone() img_url=img_url_samourai.clone() img_alt=img_alt_samourai.clone()
                                text_color=text_color_samourai.clone()
                                />
                </Show> 

                <WalletButton on_click = move |_| {set_blue_clicked(true);
                        set_blue_details(true)}
                        blue=true samourai=false green=false platform=platform()
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
pub fn BeginnerPageAndroid(cx: Scope) -> impl IntoView {

    let intro_text: String = "Controlling a bitcoin private key grants absolute authority over the
        associated bitcoin, embodying the ethos of the bitcoin movement. Self custody and personal
        responsibility restore power and sovereignty, eliminating reliance on third parties,
        particularly the state.".to_string();

    let title = "Android Self-Custody Guide".to_string();
    let quote = "Trusted Third Parties are Security Holes".to_string();
    let quote_author = "-Nick Szabo".to_string();
        
    view! { cx,
        <BeginnerPageTemplate title=title quote=quote quote_author=quote_author intro=intro_text/>
    }
    
}

/// Renders the beginner IOS page.
#[component]
#[allow(non_snake_case)]
pub fn BeginnerPageIOS(cx: Scope) -> impl IntoView {

        let intro_text: String = "Controlling a bitcoin private key grants absolute authority over the
            associated bitcoin, embodying the ethos of the bitcoin movement. Self custody and personal
            responsibility restore power and sovereignty, eliminating reliance on third parties,
            particularly the state.".to_string();

        let title = "Beginner - IOS Self-Custody Guide".to_string();
        let quote = "Trusted Third Parties are Security Holes".to_string();
        let quote_author = "-Nick Szabo".to_string();

    view! { cx,
            <BeginnerPageTemplate title=title quote=quote quote_author=quote_author intro=intro_text/> 
    
    }
}


/// Route for the android instructions - renders either bluewallet or samouraiwallet
/// depends on button clicked. 
#[component]
#[allow(non_snake_case)]
pub fn BeginnerWalletInstructions(cx: Scope, blue: bool, samourai: bool, green: bool, ios: bool) -> impl IntoView

    {
        let google_play_logo = "./../../google-play-logo.avif".to_string(); 
        let google_play_alt = "Google Play Logo".to_string();

        let apple_store_logo = "./../../apple_store_logo.svg".to_string(); 
        let apple_store_logo_2 = "./../../download_on_app_store.svg".to_string(); 
        let apple_store_alt = "Apple Store Logo".to_string();

        let img_url_github = "./../../github-mark.png".to_string();
        let img_alt_github = "Github Logo".to_string();
        
        let img_url_samourai_fdroid = "./../../F-Droid_Logo.webp".to_string();
        let img_alt_samourai_fdroid = "F-Droid Logo".to_string();
        
        let blue_google_play = r"https://play.google.com/store/apps/details?id=io.bluewallet.bluewallet".to_string();
        let blue_apple_store = r"https://apps.apple.com/app/bluewallet-bitcoin-wallet/id1376878040".to_string();
        let blue_android_apk = r"https://github.com/BlueWallet/BlueWallet/releases".to_string();

        let samourai_google_play = r"https://play.google.com/store/apps/details?id=com.samourai.wallet&hl=en_US&gl=US".to_string();
        let samourai_android_apk = r"https://samouraiwallet.com/download".to_string();
        let samourai_fdroid = r"https://samouraiwallet.com/download/fdroid".to_string();

        let green_apple_store = r"https://apps.apple.com/us/app/green-bitcoin-wallet/id1402243590".to_string();
        
        let samourai_faq = vec![
                FAQ::new_faq(1, "hello".to_string(), "Contents".to_string()),
                FAQ::new_faq(2, "YO YO YO".to_string(), "MORE CONTENT".to_string())
        ];

        
        
        //window_event_listener(ev::animationend, move |_ev| {
        //    log!("The animation ended");
        //    //log!("guide_setter {:?}", guide_setter);
        //    set_show_content(true);
        //});


        if blue {
            // Render Blue Wallet instructions 
            view! {cx, 
                <div class="flex flex-col max-w-3xl p-4 mx-auto shadow-xl rounded-xl animate-slidein">
                    <h1 class="flex justify-center text-[36px] font-bold text-blue">"Blue Wallet"</h1>
                    <br></br>
                    <div class="flex flex-col items-center">
                        <p class="text-white items-center px-4">
                            "Radically Simple 👩‍🎤 Extremely Powerful."
                        </p>
                        <p class="text-white items-center">
                            "A freedom and self-sovereign tool, disguised as a cute little Blue app in your pocket."
                        </p>                                                    
                    </div>
                    <br></br>
                    <h2 class="flex justify-center font-bold text-2xl text-white py-2">"Download Options"</h2>

                    <div class="flex flex-col mx-auto justify-center px-6 py-2 max-w-2xl mx-auto gap-4">
                        <Show
                            when=move || ios
                            fallback=move |_| view!{cx,

                            <DownloadButton href=blue_google_play.clone() logo=google_play_logo.clone() alt_txt=google_play_alt.clone() button_name="Google Play".to_string()/>
                            <DownloadButton href=blue_android_apk.clone() logo=img_url_github.clone() alt_txt=img_alt_github.clone() button_name="Android APK".to_string()/>
                                }>
                            <DownloadButton href=blue_apple_store.clone() logo=apple_store_logo_2.clone() alt_txt=apple_store_alt.clone()/>
                        </Show>        
                    </div>
                    
                    <h2 class="flex justify-center font-bold text-2xl text-white pt-6 pb-2">"Help Me!"</h2>
                    <AccordionMenu faqs=samourai_faq/>
                </div>
            }
        } else if samourai {
            // Render Samourai wallet instructions
            view! {cx, 
                <div class="flex flex-col max-w-3xl p-4 mx-auto shadow-xl rounded-xl animate-slidein">
                    <div class="flex flew-row justify-center">
                        <h1 class="flex justify-center text-[36px] font-bold text-[#3a1517ff]">"Samourai Wallet"</h1>
                    </div>
                    <div class="flex flex-col items-center">
                        <p class="text-white text-center px-4">
                            "A modern bitcoin wallet hand forged to keep your transactions private your identity masked and your funds secured."
                        </p>
                    </div>
                    <br></br>
                    <h2 class="flex justify-center font-bold text-3xl text-white py-2">"Download Options"</h2>
                    <div class="flex flex-col justify-center px-6 py-2 max-w-2xl mx-auto space-y-4">
                        <DownloadButton href=samourai_google_play.clone() logo=google_play_logo.clone() alt_txt=google_play_alt.clone() button_name="Google Play".to_string()/>
                        <DownloadButton href=samourai_fdroid.clone() logo=img_url_samourai_fdroid.clone() alt_txt=img_alt_samourai_fdroid.clone() button_name="F-Droid".to_string()/>
                        <DownloadButton href=samourai_android_apk.clone() logo=img_url_github.clone() alt_txt=img_alt_github.clone() button_name="Android APK".to_string()/>
                    </div>

                    <h2 class="flex justify-center font-bold text-2xl text-white pt-6 pb-2">"Help Me!"</h2>
                    <AccordionMenu faqs=samourai_faq/>
                </div>
            }
        } else {
            // Render BlockStream wallet instructions
            view! {cx, 
                <div class="flex flex-col max-w-3xl p-4 mx-auto shadow-xl rounded-xl animate-slidein">
                    <div class="flex flew-row justify-center">
                        <h1 class="flex justify-center text-[36px] font-bold text-[#0a7b46ff]">"BlockStream Green Wallet"</h1>
                    </div>
                    <div class="flex flex-col items-center">
                        <p class="text-white text-center px-4">
                            "Blockstream Green is a simple and secure Bitcoin wallet that makes it easy to get started sending and receiving Bitcoin."
                        </p>
                    </div>
                    <br></br>
                    <h2 class="flex justify-center font-bold text-3xl text-white py-2">"Download Options"</h2>
                    <div class="flex flex-col justify-center px-6 py-2 max-w-2xl mx-auto space-y-4">
                        <DownloadButton href=green_apple_store.clone() logo=apple_store_logo_2.clone() alt_txt=apple_store_alt.clone()/>
                    </div>

                    <h2 class="flex justify-center font-bold text-2xl text-white pt-6 pb-2">"Help Me!"</h2>
                    <AccordionMenu faqs=samourai_faq/>
                </div>
                }
        }
    
    }
