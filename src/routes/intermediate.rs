use crate::extras::accordion_menu::AccordionMenu;
use crate::extras::generic_button::*;
use leptos::*;

#[component]
#[allow(non_snake_case)]
pub fn IntermediateIntroPage() -> impl IntoView {
    let title = "Intermediate Self-Custody Guide".to_string();
    let quote = "Rights Are Not Given, They Are Taken".to_string();
    let quote_author = "-Aldous Huxley".to_string();

    let explanation: String = "We'll start by setting up a ColdCard signing device, and connecting it to Sparrow. I recommend following ColdCard's Paranoid Guide, however,
        you're welcome to choose the Ultra Quick or Middle Ground guide if you prefer. In Part two we'll decide which bitcoin node setup we want to use, and then connect our Sparrow wallet to it.
        Once we're through with this, you'll have an excellent, secure and private bitcoin self-custody solution.
        ".to_string();

    view! {
        <div id="basic" class="flex flex-col max-w-3xl mx-auto rounded-xl pb-10 animate-fadeinone" >
            <div class="flex flex-col p-6 pt-10 max-w-3xl mx-auto">
                    <h1 class="flex justify-center text-[36px] text-white font-semibold">{title}</h1>
                <div class="flex justify-start pt-4 max-w-sm">
                    <p class="text-lg text-white italic">{quote}</p>
                </div>
                <div class="flex max-w-sm">
                    <p class="text-sm text-white italic">{quote_author}</p>
                </div>
            </div>

            <div class="flex flex-col p-6 max-w-3xl mx-auto bg-[#123c64] rounded-xl shadow-xl" >
                <p class="font-bold text-white">"Coldcard & Node Setup:"</p>
                <p class="pb-2 text-white">""</p>
                <p class="mr-4 text-md text-white">
                    "It's time to take your bitcoin privacy and security to another level.
                    In this guide we'll build on our previous basic desktop setup. If you
                    originally chose a mobile setup, I recommend that you first start with the"
                    <a class="text-[#8cb4ff] underline-offset-auto" href="/guides/basic/desktop">" basic desktop guide "</a>
                    "before continuing."
                </p>
                <p class="pt-2 text-md text-white">{explanation}</p>
            </div>

            <div class="mx-auto max-w-xl p-4 w-full" >
                <div class="mx-auto border border-solid border-gray-400"></div>
            </div>

            <div class="pb-6 pt-4">
                <GenericButton path="/guides/intermediate/hardware-wallet".to_string() wallet_title="Level up to Intermediate".to_string() img_url="./../../../increase.png".to_string() img_alt="Arrow icon created by Pixel perfect".to_string()/>
            </div>

        </div>

    }
}

#[component]
#[allow(non_snake_case)]
pub fn IntermediateHardwarePage() -> impl IntoView {
    let title = "Step 1 - Hardware Wallet Setup".to_string();

    view! {

        <div id="hardware_page" class="flex flex-col max-w-3xl mx-auto rounded-xl p-5 animate-fadeinone" >
            <div class="flex flex-col p-6 pt-10 max-w-3xl mx-auto">
                <h1 class="flex justify-center text-xl text-white font-semibold">{title}</h1>
            </div>
            // Might want to display this flex horizontal for desktop
            <div class="pb-2 pt-2">
                <GenericExternalButton path="https://store.coinkite.com/store/bundle-mk4-basic".to_string() wallet_title="Buy a ColdCard".to_string()
                                    img_url="./../../../coldcard-logo-nav.png".to_string() img_alt="coldcard logo".to_string()
                                    new_width="12".to_string() new_height="8".to_string()/>
            </div>
            <div class="pb-2 pt-2">
                <GenericExternalButton path="https://store.coinkite.com/store/seedplate".to_string() wallet_title="Buy a Seedplate".to_string()
                                    img_url="./../../../steel.png".to_string() img_alt="Steel plate".to_string()
                                    new_width="10".to_string() new_height="8".to_string()/>
            </div>
            <div class="pb-2 pt-2">
                <GenericExternalButton path="https://store.coinkite.com/store/drillpunch".to_string() wallet_title="Buy a Center Punch".to_string()
                                    img_url="./../../../hole-puncher.png".to_string() img_alt="Hole puncher icons created by Smashicons - Flaticon".to_string()
                                    new_width="10".to_string() new_height="8".to_string()/>
            </div>

            <h2 class="flex justify-center font-bold text-xl text-white pt-6 pb-2">"Instructions"</h2>
            <AccordionMenu faq_name="hardware_wallet_setup".to_string()/>

            <h2 class="flex justify-center font-bold text-xl text-white pt-6 pb-2">"Step 2 - Node Setup"</h2>
            <div class="pb-4 pt-6">
                <GenericButton path="/guides/intermediate/node".to_string() wallet_title="Running Bitcoin".to_string() img_url="./../../../bitcoin_server.png".to_string() img_alt="Arrow icon created by Pixel perfect".to_string()/>
            </div>
        </div>
    }
}

#[component]
#[allow(non_snake_case)]
pub fn IntermediateNodePage() -> impl IntoView {
    let title = "Step 2 - Node Setup".to_string();

    view! {
        <div id="hardware_page" class="flex flex-col max-w-4xl mx-auto rounded-xl p-5 animate-fadeinone" >
            <div class="flex flex-col p-6 pt-10 max-w-4xl mx-auto">
                <h1 class="flex justify-center text-xl text-white font-semibold">{title}</h1>
            </div>
            <div class="inline-flex flex-col md:flex-row gap-2">
                <div>
                    <GenericImageSubTextButton path="https://store.coinkite.com/store/bundle-mk4-basic".to_string() title="Start9".to_string() short_desc="Sovereign Computing".to_string()/>
                </div>
                <div>
                    <GenericImageSubTextButton path="https://store.coinkite.com/store/seedplate".to_string() title="RaspiBlitz".to_string() short_desc="Not Your Node, Not Your Rules.".to_string()/>
                </div>
                <div>
                    <GenericImageSubTextButton path="https://store.coinkite.com/store/drillpunch".to_string() title="MyNode".to_string() short_desc="Bitcoin, Lightning, and more!".to_string()/>
                </div>
            </div>
            <h2 class="flex justify-center font-bold text-xl text-white pt-6 pb-2">"Instructions"</h2>
            <AccordionMenu faq_name="node_setup".to_string()/>
        </div>
    }
}
