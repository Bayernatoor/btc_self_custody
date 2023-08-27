use leptos::{*, ev::MouseEvent};
use leptos::ev::{Event, AnimationEvent};
use crate::routes::beginner_wallet_instructions::*; 

#[component]
pub fn WalletButton<F>(cx: Scope, on_click: F, wallet_name: String, short_desc: String, img_url:
                        String, img_alt: String, text_color: String) -> impl IntoView
    where
        F: Fn(MouseEvent) + 'static,
    {
    
    view! {cx, 
        <button class="flex justify-center shrink-0 h-20 w-72 p-4 mx-auto bg-white rounded-xl items-center space-x-4" on:click=on_click>
          <div class="shrink-0">
            <img class="h-12 w-12" src={img_url} alt={img_alt}/>
          </div>
          <div>
            <h3 class=format!("text-xl font-medium text-[{text_color}]")>{wallet_name}</h3>
            <p class="text-slate-500">{short_desc}</p>
          </div>
        </button>
    }
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

    create_effect(cx, move |_| {
        let beginner_guide_element = window().document().unwrap().get_element_by_id("beginner");
        log!("guide element: {:?}", beginner_guide_element);
    });

    
    window_event_listener(ev::animationend, move |_e| {
            set_slideout_ends(true);
            log!("Animation Done");
     });   

    view! { cx, 
        <div id="beginner" class="flex flex-col max-w-3xl mx-auto shadow-xl rounded-xl pb-10" class=("animate-slideout", move || samourai_details() || blue_details() == true)>
            <div class="flex flex-col p-6 pt-10 max-w-3xl mx-auto " class=("hidden", move || slideout_ends() == true)>
                    <h1 class="flex justify-center text-[36px] text-white">{title}</h1>
                <div class="flex justify-end pt-10 max-w-sm">
                    <p class="text-lg text-white">{quote}</p>
                </div>
                <div class="flex justify-end max-w-sm">
                    <p class="text-lg text-white">{quote_author}</p>
                </div>
            </div> 

            <div class="flex flex-col p-6 max-w-2xl mx-auto" class=("hidden", move || slideout_ends() == true)>
                <p class="font-bold text-white">"Bitcoin Self-Custody:"</p>
                <p class="pb-2 text-white">"The act of taking possession of a bitcoin private key."</p>
                <p class="mr-4 text-lg text-white">{intro}</p>
            </div> 
            
            <div class="mx-auto max-w-xl p-4 w-full" class=("hidden", move || slideout_ends() == true)>
                <div class="mx-auto border border-dashed border-gray-400"></div>
            </div>

            <div class="flex flex-col mx-auto justify-center" class=("hidden", move || slideout_ends() == true)>
                <p class="flex justify-center text-center mx-auto max-w-2xl text-2xl font-bold text-white" >"Alright! Let's get started."</p>
                <h2 class="flex justify-center py-4 max-w-2xl text-center mx-auto text-xl font-bold text-white" >"Pick A Wallet"</h2>
            </div>
            <div class="flex flex-col md:flex-row px-6 py-2 max-w-2xl mx-auto gap-4">
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
                    <BeginnerAndroidWalletInstructions blue=blue_details() samourai=samourai_details()/> 
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

    let title = "Android Self-Custody Guide".to_string();
    let quote = "Trusted Third Parties are Security Holes".to_string();
    let quote_author = "-Nick Szabo".to_string();

    view! { cx,
        <BeginnerPageTemplate title=title quote=quote quote_author=quote_author intro=intro_text/> 
    }
    



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
