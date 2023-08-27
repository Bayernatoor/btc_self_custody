use leptos::*;


#[component]
pub fn BeginnerAndroidWalletInstructions(cx: Scope, blue: bool, samourai: bool) -> impl IntoView

    {
        let google_play_logo = "./../../google-play-logo.avif".to_string(); 
        let google_play_alt = "Google Play Logo".to_string();
        let img_url_blue_github = "./../../github-mark.png".to_string();
        let img_alt_blue_github = "Github Logo".to_string();
        
        let img_url_samourai_FDroid = "./../../F-Droid_Logo.webp".to_string();
        let img_alt_samourai_FDroid = "F-Droid Logo".to_string();

        let (show_content, set_show_content) = create_signal(cx, false);
        let (reload_page, set_reload_page) = create_signal(cx, false);
        
        window_event_listener(ev::animationend, move |_ev| {
            log!("The animation ended");
            set_show_content(true);
        });

        create_effect(cx, move |_| {
            if reload_page() == true {
                let _ = window().location().reload();
                log!("reload window");
            }
        });

        if blue {
            view! {cx, 
                <div class="flex flex-col max-w-3xl py-4 mx-auto rounded-xl" class=("animate-slidein", move || show_content() == true) class:hidden=move || show_content() == false>
                    <h1 class="flex justify-center text-[36px] font-bold text-blue">"Blue Wallet"</h1>
                    <br></br>
                    <div class="flex flex-col items-center">
                        <p class="text-white items-center">
                            "Radically Simple 👩‍🎤 Extremely Powerful."
                        </p>
                        <p class="text-white items-center">
                            "A freedom and self-sovereign tool, disguised as a cute little Blue app on your pocket."
                        </p>                                                    
                    </div>
                    <br></br>
                    <h2 class="flex justify-center font-bold text-2xl text-white py-2">"Download Options:"</h2>
                    <div class="flex flex-col mx-auto justify-center px-6 py-2 max-w-2xl mx-auto gap-4">
                        <button class="flex justify-center p-2 shrink-0 h-18 w-60 mx-auto bg-white rounded-xl items-center space-x-4">
                            <div class="shrink-0">
                                <img class="h-12 w-12" src=format!("{}", google_play_logo) alt=format!("{}", google_play_alt) />
                            </div>
                            <div class="font-bold">
                                <a href=r"https://play.google.com/store/apps/details?id=io.bluewallet.bluewallet"
                                        target="_blank" rel="external">"Google Play"</a>
                            </div>
                        </button>

                        <button class="flex justify-center p-2 shrink-0 h-18 w-60 mx-auto bg-white rounded-xl flex items-center space-x-4">
                            <div class="shrink-0">
                                <img class="h-12 w-12" src=format!("{}", img_url_blue_github) alt=format!("{}", img_alt_blue_github) />
                            </div>
                            <div class="font-bold">
                                <a href=r"https://github.com/BlueWallet/BlueWallet/releases"
                                        target="_blank" rel="external">"Android APK"</a>
                            </div>
                        </button>
                    </div>
                </div>
            }
        } else {
            view! {cx, 
                <div class="flex flex-col max-w-3xl py-4 mx-auto rounded-xl" class=("animate-slidein", move || show_content() == true) class:hidden=move || show_content() == false>
                    <div class="flex flew-row justify-center">
                        <button>"Back"</button>
                        <h1 class="flex justify-center text-[36px] font-bold text-black">"Samourai Wallet"</h1>
                    </div>
                    <br></br>
                    <p class="text-white">
                        "Although Samourai has some very advanced features, it is, in my
                        opinion, one of the best wallets available. What’s great is you can
                        use it as a simple bitcoin wallet and as you continue on your self
                        custody journey you’ll have easy access to its advanced
                        features."
                    </p>
                    <br></br>
                    <p class="text-white">
                        "After opening the app, select “mainnet” and
                        continue. Follow the prompts to create your wallet. Make sure to
                        read the instructions and take your time. Understanding the process
                        is important on your journey to self custody."
                    </p>
                    <br></br>
                    <h2 class="flex justify-center font-bold text-3xl text-white py-2">"Download Options:"</h2>
                    <div class="flex flex-col justify-center px-6 py-2 max-w-2xl mx-auto space-y-4">
                        <button class="flex place-content-evenly p-2 shrink-0 h-18 w-60 mx-auto bg-white rounded-xl flex items-center space-x-4">
                            <div class="shrink-0">
                                <img class="h-12 w-12" src=format!("{}", google_play_logo) alt=format!("{}", google_play_alt) />
                            </div>
                            <div class="font-bold">
                                <a href=r"https://play.google.com/store/apps/details?id=com.samourai.wallet&hl=en_US&gl=US"
                                target="_blank" rel="external">"Google Play"</a>
                            </div>
                        </button>

                        <button class="flex place-content-evenly p-2 shrink-0 h-18 w-60 mx-auto bg-white rounded-xl flex items-center space-x-4">
                            <div class="shrink-0">
                                <img class="h-12 w-12" src=format!("{}", img_url_samourai_FDroid) alt=format!("{}", img_alt_samourai_FDroid) />
                            </div>
                            <div class="font-bold">
                                <a href=r"https://samouraiwallet.com/download"
                                target="_blank" rel="external">"F-Droid"</a>
                            </div>
                        </button>

                        <button class="flex place-content-evenly p-2 shrink-0 h-18 w-60 mx-auto bg-white rounded-xl flex items-center space-x-4">
                            <div class="shrink-0">
                                <img class="h-12 w-12" src=format!("{}", img_url_blue_github) alt=format!("{}", img_alt_blue_github) />
                            </div>
                            <div class="font-bold">
                                <a href=r"https://samouraiwallet.com/download" target="_blank" rel="external">"Android APK"</a>
                            </div>
                        </button>
                    </div>
                </div>

            }}
    }
