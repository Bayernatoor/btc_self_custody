use leptos::*;
use leptos_router::*;

//#[component]
//fn

// Renders the beginner Android page
#[component]
pub fn BeginnerPageAndroid(cx: Scope) -> impl IntoView {
    view! { cx,
        <header class="beginner-guide-header">
            <h1>"Beginner - Android Self-Custody Guide"</h1>
        </header>
        <div class="quote">
             <p>"Trusted Third Parties are Security Holes"</p>
        <div class="quoted">
             <p>"-Nick Szabo"</p>
        </div>
        </div>
        <div class="guide-container">
            <div class="beginner-section">
                <p>"<b>Bitcoin Self-Custody:</b> The act of taking possession of a bitcoin private key."</p>
                <p>"Having control of a bitcoin private key gives you absolute authority over the bitcoin associated with this key. This is, in my opinion, the ethos behind the bitcoin movement. Self custody is necessary, personal responsibility over your money and therefore your time puts the power back in your hands and makes you self-sovereign. This ultimately removes your dependence on third parties and most importantly the state."</p>
            <br></br>
            <p>"<b>Recommended wallet:</b> Samourai wallet (privacy focused)"</p>
            <p>"<b>Download Options:</b>"</p>
            <ol class="android-download-links">
                <li><a href=r"https://play.google.com/store/apps/details?id=com.samourai.wallet&hl=en_US&gl=US" target="_blank">"Google Play - Beginner Friendly"</a></li>
                <li><a href=r"https://samouraiwallet.com/download" target="_blank">"F-Droid"</a></li>
                <li><a href=r"https://samouraiwallet.com/download" target="_blank">"Android APK"</a></li>
            </ol>
            <br></br>
            <p>"Although Samourai has some very advanced features, it is, in my opinion, one of the best wallets available. What’s great is you can use it as a simple bitcoin wallet and as you continue on your self custody journey you’ll have easy access to its advanced features."</p>
            <p>"After opening the app, select “mainnet” and continue. Follow the prompts to create your wallet. Make sure to read the instructions and take your time. Understanding the process is important on your journey to self custody."</p>
            <br></br>
            <h2>"Samourai Wallet FAQs"</h2>
            <div class="accordion">
                <input type="checkbox" id="toggle1" class="accordion-toggle" />
                <label for="toggle1" class="accordion-title">"Additional features"</label>
                <div class="accordion-content">
                  <p>"Since you’re just starting out on your self-custody journey let's keep things simple. If you’re asked to make a decision regarding features that you do not understand - for example: turn on Tor - simply leave it on the default value and move. The intermediate guide will dive into those.
    "</p>
                </div>

                <input type="checkbox" id="toggle2" class="accordion-toggle" />
                <label for="toggle2" class="accordion-title">"PassPhrase"</label>
                <div class="accordion-content">
                  <p>"Content for Section 2"</p>
                </div>

                <input type="checkbox" id="toggle3" class="accordion-toggle" />
                <label for="toggle3" class="accordion-title">"Pin Code"</label>
                <div class="accordion-content">
                  <p>"Content for Section 3"</p>
                </div>

                <input type="checkbox" id="toggle3" class="accordion-toggle" />
                <label for="toggle3" class="accordion-title">"Secret Words"</label>
                <div class="accordion-content">
                  <p>"Content for Section 3"</p>
                </div>

                <input type="checkbox" id="toggle3" class="accordion-toggle" />
                <label for="toggle3" class="accordion-title">"Paynym"</label>
                <div class="accordion-content">
                  <p>"Content for Section 3"</p>
                </div>

                <input type="checkbox" id="toggle3" class="accordion-toggle" />
                <label for="toggle3" class="accordion-title">"Samourai Docs"</label>
                <div class="accordion-content">
                  <p>"Content for Section 3"</p>
                </div>
            </div>
            </div>
        </div>
    }
}

/// Renders the beginner IOS page.
#[component]
pub fn BeginnerPageIOS(cx: Scope) -> impl IntoView {
    view! { cx,
        <h1 color="black">"Beginner Section"</h1>
        <img src="./bitcoin_log.png" alt="bitcoin logo" />
    }
}
