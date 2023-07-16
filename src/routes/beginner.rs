use leptos::{*, ev::MouseEvent, ev::animationend};


#[component]
pub fn WalletButton<F>(cx: Scope, on_click: F, wallet_name: String, short_desc: String, img_url:
                        String, img_alt: String, text_color: String) -> impl IntoView
    where
        F: Fn(MouseEvent) + 'static,
    {
    
    view! {cx, 
        <button class=format!("p-6 max-w-sm mx-auto bg-white rounded-xl shadow-md flex items-center space-x-4") on:click=on_click>
          <div class="shrink-0">
            <img class="h-12 w-12" src={img_url} alt={img_alt}/>
          </div>
          <div>
            <div class=format!("text-xl font-medium text-[{text_color}]")>{wallet_name}</div>
            <p class="text-slate-500">{short_desc}</p>
          </div>
        </button>
    }
}       

#[component]
fn WalletInstructions(cx: Scope, blue: bool, samourai: bool) -> impl IntoView

    {
        
        let (show_content, set_show_content) = create_signal(cx, false);

        window_event_listener("animationend", move |ev| {
            log!("The animation ended");
            set_show_content(true);
        });

        if blue {
            view! {cx, 
                <div id="guide-container" class="max-w-6xl py-4 mx-auto rounded-xl" class=("animate-slidein", move || show_content() == true) class:hidden=move || show_content() == false>
                    <p class="font-bold">"Download Options:"</p>
                    <ol class="android-download-links">
                    <li>
                        <a href=r"https://play.google.com/store/apps/details?id=com.samourai.wallet&hl=en_US&gl=US"
                        target="_blank">"Google Play - Beginner Friendly"</a></li>
                    <li>
                        <a href=r"https://samouraiwallet.com/download"
                        target="_blank">"F-Droid"</a></li>
                    <li>
                        <a href=r"https://samouraiwallet.com/download" target="_blank">"Android APK"</a></li>
                    </ol>
                </div>
            }
        } else {
            view! {cx, 
                <div id="guide-container" class="max-w-6xl py-4 mx-auto rounded-xl" class=("animate-slidein", move || show_content() == true) class:hidden=move || show_content() == false>
                    <p class="font-bold" >"Download Options:"</p>
                    <ol class="android-download-links">
                    <li>
                        <a href=r"https://play.google.com/store/apps/details?id=com.samourai.wallet&hl=en_US&gl=US"
                        target="_blank">"Google Play - Beginner Friendly"</a></li>
                    <li>
                        <a href=r"https://samouraiwallet.com/download"
                        target="_blank">"F-Droid"</a></li>
                    <li>
                        <a href=r"https://samouraiwallet.com/download" target="_blank">"Android APK"</a></li>
                    </ol>
                </div>

            }}
    }

#[component]
pub fn BeginnerPageTemplate(cx: Scope, title: String, quote: String, quote_author: String, intro:
                            String 
                            ) -> impl IntoView {

    let (samourai_clicked, set_samourai_clicked) = create_signal(cx, false);
    let (blue_clicked, set_blue_clicked) = create_signal(cx, false);
    let (samourai_details, set_samourai_details) = create_signal(cx, false);
    let (blue_details, set_blue_details) = create_signal(cx, false);
    
    let (slideout_ends, set_slideout_ends) = create_signal(cx, false);

    let wallet_name_samourai = "Samourai Wallet".to_string();
    let short_desc_samourai = "Privacy + Freedom Tools".to_string();
    let img_url_samourai = "./../../samourai_logo.png".to_string();
    let img_alt_samourai = "Samourai Logo".to_string();
    let text_color_samourai = "#1a578f".to_string(); // actual colour should be: #c0272b

    let wallet_name_blue = "Blue Wallet".to_string();
    let short_desc_blue = "Basic + Ease of Use".to_string();
    let img_url_blue = "./../../bluewallet_logo.webp".to_string();
    let img_alt_blue = "Blue Wallet".to_string();
    let text_color_blue = "#1a578f".to_string();

    //let beginner_guide_element = document().get_element_by_id("beginner_guide").unwrap(); 

    //let beginner_guide_element = document

    
    //beginner_guide_element.on::<AnimationEvent>("animationend", |_event| {
    //    log!("The animation ended");
    //    set_slideout_ends(true);
    //});

   // window_event_listener("animationend", move |ev| {
   //     log!("the animation ended");
   //     set_slideout_ends(true);
   // });
   //
   //
   //let beginner_guide_element = document()
   // .get_element_by_id("beginner_guide")
   // .unwrap()
   // .dyn_into::<Element>()
   // .unwrap();

   // let event_listener = Closure::wrap(Box::new(move |event: Event| {
   //     log!("The animation ended");
   //     set_slideout_ends(true);
   // }) as Box<dyn FnMut(_)>);
   // 
   // beginner_guide_element
   //     .add_event_listener_with_callback("animationend", event_listener.as_ref().unchecked_ref())
   //     .unwrap();
   // 
   // event_listener.forget();
    
 
    view! { cx, 
        <div id="beginner_guide" class="max-w-6xl mx-auto rounded-xl" class=("animate-slideout", move || samourai_details() || blue_details() == true) >
            <div class="flex flex-col p-6 pt-10 max-w-2xl mx-auto" class=("hidden", move || slideout_ends() == true)>
                <h1 class="flex justify-center text-3xl items-center font-bold text-black">{title}</h1>
                <div class="flex justify-end pt-10 max-w-sm">
                    <p class="mr-4 text-lg text-black">{quote}</p>
                </div>
                <div class="flex justify-end max-w-sm">
                    <p class="mr-4 text-lg text-black">{quote_author}</p>
                </div>
            </div> 

            <div class="flex flex-col p-6 max-w-2xl mx-auto" class=("hidden", move || slideout_ends() == true)>
                <p class="font-bold">"Bitcoin Self-Custody:"</p><p class="pb-2">"The act of taking possession of a bitcoin private key."</p>
                <p class="mr-4 text-lg text-black">{intro}</p>
            </div>
            
            <h2 class="flex justify-center ps-8 py-2 max-w-2xl text-center text-xl font-bold text-black" class=("hidden", move || slideout_ends() == true)>"Select Your Wallet: "</h2>
            <div class="flex flex-row px-6 py-2 max-w-2xl mx-auto">
                <Show
                    when=move || samourai_clicked() || blue_clicked()
                    fallback=move |cx| view! { cx, 
                    <WalletButton on_click = move |_| {set_samourai_clicked(true);
                                    set_blue_clicked(true); set_samourai_details.set(true)}
                        wallet_name=wallet_name_samourai.clone() short_desc=short_desc_samourai.clone() img_url=img_url_samourai.clone() img_alt=img_alt_samourai.clone()
                        text_color=text_color_samourai.clone()
                    />

                    <WalletButton on_click = move |_| {set_blue_clicked(true);
                                      set_samourai_clicked(true); set_blue_details.set(true)}
                        wallet_name=wallet_name_blue.clone() short_desc=short_desc_blue.clone() img_url=img_url_blue.clone()
                        img_alt=img_alt_blue.clone() text_color=text_color_blue.clone()
                    />}
                >
                    <WalletInstructions blue=blue_details() samourai=samourai_details()/> 
                </Show>
            </div> 
        </div> 
    }    
}


// Renders the beginner Android page
#[component]
pub fn BeginnerPageAndroid(cx: Scope) -> impl IntoView {


    let intro_text: String = "Controlling a bitcoin private key grants absolute authority over the
        associated bitcoin, embodying the ethos of the bitcoin movement. Self custody and personal
        responsibility restore power and sovereignty, eliminating reliance on third parties,
        particularly the state.".to_string();

    let title = "Beginner - Android Self-Custody Guide".to_string();
    let quote = "Trusted Third Parties are Security Holes".to_string();
    let quote_author = "-Nick Szabo".to_string();

    view! { cx,
        <BeginnerPageTemplate title=title quote=quote quote_author=quote_author intro=intro_text/> 
    }
    

        //        <p>"<b>Download Options:</b>"</p>
        //        <ol class="android-download-links">
        //        <li>
        //            <a href=r"https://play.google.com/store/apps/details?id=com.samourai.wallet&hl=en_US&gl=US"
        //            target="_blank">"Google Play - Beginner Friendly"</a></li>
        //        <li>
        //            <a href=r"https://samouraiwallet.com/download"
        //            target="_blank">"F-Droid"</a></li>
        //        <li>
        //            <a href=r"https://samouraiwallet.com/download" target="_blank">"Android APK"</a></li>
        //        </ol>

        //        <br></br>
        //        <p>
        //            "Although Samourai has some very advanced features, it is, in my
        //            opinion, one of the best wallets available. What’s great is you can
        //            use it as a simple bitcoin wallet and as you continue on your self
        //            custody journey you’ll have easy access to its advanced
        //            features."
        //        </p>

        //        <p>
        //            "After opening the app, select “mainnet” and
        //            continue. Follow the prompts to create your wallet. Make sure to
        //            read the instructions and take your time. Understanding the process
        //            is important on your journey to self custody."
        //        </p>
        //        <br></br>
        //    <h2>"Samourai Wallet FAQs"</h2>




        //    <div class="accordion">
        //        <input type="checkbox" id="toggle1" class="accordion-toggle" />
        //        <label for="toggle1" class="accordion-title">"Additional features"</label>
        //        <div class="accordion-content">
        //          <p>
        //          "Since you’re just starting out on your self-custody
        //          journey let's keep things simple. If you’re asked to make a
        //          decision regarding features that you do not understand - for
        //          example: turn on Tor - simply leave it on the default value
        //          and move. The intermediate guide will dive into those. "
        //          </p>
        //        </div>
        //        <input type="checkbox" id="toggle2" class="accordion-toggle" />
        //        <label for="toggle2" class="accordion-title">"PassPhrase"</label>
        //        <div class="accordion-content">
        //          <p>"Content for Section 2"</p>
        //        </div>

        //        <input type="checkbox" id="toggle3" class="accordion-toggle" />
        //        <label for="toggle3" class="accordion-title">"Pin Code"</label>
        //        <div class="accordion-content">
        //          <p>"Content for Section 3"</p>
        //        </div>

        //        <input type="checkbox" id="toggle3" class="accordion-toggle" />
        //        <label for="toggle3" class="accordion-title">"Secret Words"</label>
        //        <div class="accordion-content">
        //          <p>"Content for Section 3"</p>
        //        </div>

        //        <input type="checkbox" id="toggle3" class="accordion-toggle" />
        //        <label for="toggle3" class="accordion-title">"Paynym"</label>
        //        <div class="accordion-content">
        //          <p>"Content for Section 3"</p>
        //        </div>

        //        <input type="checkbox" id="toggle3" class="accordion-toggle" />
        //        <label for="toggle3" class="accordion-title">"Samourai Docs"</label>
        //        <div class="accordion-content">
        //          <p>"Content for Section 3"</p>
        //        </div>
        //    </div>
        //    </div>
        //</div>
    //}
}

/// Renders the beginner IOS page.
#[component]
pub fn BeginnerPageIOS(cx: Scope) -> impl IntoView {

        let intro_text: String = "Controlling a bitcoin private key grants absolute authority over the
            associated bitcoin, embodying the ethos of the bitcoin movement. Self custody and personal
            responsibility restore power and sovereignty, eliminating reliance on third parties,
            particularly the state.".to_string();

        let title = "Beginner - Android Self-Custody Guide".to_string();
        let quote = "Trusted Third Parties are Security Holes".to_string();
        let quote_author = "-Nick Szabo".to_string();

    view! { cx,
            <BeginnerPageTemplate title=title quote=quote quote_author=quote_author intro=intro_text/> 
    
    }
}
