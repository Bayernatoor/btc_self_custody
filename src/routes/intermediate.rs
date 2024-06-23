use crate::extras::accordion_menu::AccordionMenu;
use crate::extras::generic_button::*;
use leptos::*;

#[component]
#[allow(non_snake_case)]
pub fn IntermediateIntroPage() -> impl IntoView {
    let title = "Intermediate Self-Custody Guide".to_string();
    let quote = "Rights Are Not Given, They Are Taken".to_string();
    let quote_author = "-Aldous Huxley".to_string();

    let explanation: String = "We'll start by setting up a ColdCard signing device, and connecting it to Sparrow. 
        In Part two we'll decide which bitcoin node setup we want to use, and then connect our Sparrow wallet to it.
        Once we're through with this, you'll have a standards based, secure and private bitcoin self-custody solution.
        ".to_string();

    view! {
        <div id="basic" class="grid gap-6 max-w-5xl mx-auto pb-20 animate-fadeinone grid-rows-[auto_auto_1fr] lg:gap-8">
            //- Section 1: Title, Quote, and Quote Author
            <div class="flex flex-col mx-auto px-4 pt-10 lg:pt-0">
                <h1 class="text-center text-[2.25rem] text-[#f7931a] font-semibold leading-tight lg:text-[3rem]">{title}</h1>
                <div class="text-center max-w-sm mx-auto pt-4">
                    <p class="text-lg font-semibold text-white italic">{quote}</p>
                </div>
                <div class="text-center max-w-sm mx-auto">
                    <p class="text-md text-white italic">{quote_author}</p>
                </div>
            </div>

            //- Section 2: Intro and Explanation
            <div class="px-4 lg:pt-0 lg:px-0">
                <h2 class="text-left text-[1.5rem] text-[#f7931a] font-semibold lg:text-[1.5rem]">"Coldcard & Node Setup:"</h2>
                <p class="text-lg text-white">
                    "It's time to take your bitcoin privacy and security to another level.
                    In this guide we'll build on our previous basic desktop setup. If you
                    originally chose a mobile setup, I recommend that you first start with the"
                    <a class="text-[#8cb4ff] underline-offset-auto" href="/guides/basic/desktop">" basic desktop guide "</a>
                    "before continuing."
                </p>
                <p class="pt-2 text-lg text-white">{explanation}</p>
            </div>

            // Section 3: Divider and Button
            <div class="px-4 lg:pb-4 lg:px-0">
                <hr class="border border-solid border-gray-400 mx-auto w-full mb-6"/>
                <div class="pb-6 pt-4">
                    <GenericButton path="/guides/intermediate/hardware-wallet".to_string() wallet_title="Level up to Intermediate".to_string() img_url="./../../../increase.png".to_string() img_alt="Arrow icon created by Pixel perfect".to_string()/>
                </div>
            </div>
        </div>

    }
}

#[component]
#[allow(non_snake_case)]
pub fn IntermediateHardwarePage() -> impl IntoView {
    let title = "Step 1 - Hardware Wallet Setup".to_string();

    view! {
        <div id="hardware_page" class="grid gap-6 max-w-5xl mx-auto pb-20 animate-fadeinone grid-rows-[auto_auto_1fr] lg:gap-8 lg:mt-10">
            // Section 1: Title
            <div class="flex flex-col mx-auto pt-10 lg:pt-0">
                <h1 class="text-center text-[2rem] text-[#f7931a] font-semibold leading-tight lg:text-[2.25rem]">{title}</h1>
            </div>

            // Section 2: Purchase Buttons
            <div class="px-4 lg:pt-0 lg:px-0">
                <div class="flex flex-col gap-4">
                    //- Purchase Buttons
                    <div class="flex justify-center">
                        <GenericExternalButton path="https://store.coinkite.com/store/bundle-mk4-basic".to_string() wallet_title="Buy a ColdCard".to_string()
                            img_url="./../../../coldcard-logo-nav.png".to_string() img_alt="coldcard logo".to_string()
                            new_width="24".to_string() new_height="8".to_string()/>
                    </div>
                    <div class="flex justify-center">
                        <GenericExternalButton path="https://store.coinkite.com/store/seedplate".to_string() wallet_title="Buy a Seedplate".to_string()
                            img_url="./../../../steel.png".to_string() img_alt="Steel plate".to_string()
                            new_width="10".to_string() new_height="8".to_string()/>
                    </div>
                    <div class="flex justify-center">
                        <GenericExternalButton path="https://store.coinkite.com/store/drillpunch".to_string() wallet_title="Buy a Center Punch".to_string()
                            img_url="./../../../hole-puncher.png".to_string() img_alt="Hole puncher icons created by Smashicons - Flaticon".to_string()
                            new_width="10".to_string() new_height="8".to_string()/>
                    </div>
                </div>
            </div>

            // Section 3: Divider, Instructions, and Accordion Menu
            <div class="px-4 lg:pb-4 lg:px-0">
                <hr class="border border-solid border-gray-400 mx-auto w-full mb-6"/>
                <h2 class="text-center pb-4 text-[1.5rem] text-[#f7931a] font-semibold lg:text-[1.5rem]">"Start Here"</h2>
                <AccordionMenu faq_name="hardware_wallet_setup".to_string()/>
            </div>

            // Section 4: Node Setup Button
            <div class="px-4 lg:pb-4 lg:px-0">
                <h3 class="text-center pb-4 text-[1.5rem] text-[#f7931a] font-semibold lg:text-[1.5rem]">"Step 2 - Node Setup"</h3>
                <div class="pb-4 flex justify-center">
                    <GenericButton path="/guides/intermediate/node".to_string() wallet_title="Running Bitcoin".to_string() img_url="./../../../bitcoin_server.png".to_string() img_alt="Arrow icon created by Pixel perfect".to_string()/>
                </div>
            </div>
        </div>
    }
}

#[component]
#[allow(non_snake_case)]
pub fn IntermediateNodePage() -> impl IntoView {
    let title = "Step 2 - Node Setup".to_string();

    view! {
        <div id="hardware_page" class="grid gap-6 max-w-5xl mx-auto pb-20 animate-fadeinone grid-rows-[auto_auto_1fr] lg:gap-8">
            // Section 1: Title
            <div class="flex flex-col mx-auto px-4 pt-10 lg:pt-0">
                <h1 class="text-center text-[2rem] text-[#f7931a] font-semibold leading-tight lg:text-[2.25rem]">{title}</h1>
            </div>

            // Section 2: Purchase Buttons
            <div class="px-4 lg:pt-0 lg:px-0">
                <div class="flex flex-col md:flex-row justify-center gap-4">
                    // Purchase Buttons
                    <div class="flex justify-center md:justify-end">
                        <GenericExternalButton path="https://start9.com/".to_string() wallet_title="Sovereign Computing".to_string()
                            img_url="./../../../start9_transparent_inverted.png".to_string() img_alt="Start9 logo".to_string()
                            new_width="28".to_string() new_height="".to_string()/>
                    </div>
                    <div class="flex justify-center md:justify-end">
                        <GenericExternalButton path="https://mynodebtc.github.io/".to_string() wallet_title="Bitcoin, Lightning and more!".to_string()
                            img_url="./../../../mynode_logo.png".to_string() img_alt="MyNode logo".to_string()
                            new_width="32".to_string() new_height="6".to_string()/>
                    </div>
                    <div class="flex justify-center md:justify-start">
                        <GenericExternalButton path="https://shop.fulmo.org/raspiblitz/".to_string() wallet_title="Not Your Node, Not your Rules".to_string()
                            img_url="./../../../raspiblitz_logo_main.png".to_string() img_alt="RaspiBlitz logo".to_string()
                            new_width="28".to_string() new_height="".to_string()/>
                    </div>
                </div>
            </div>

            // Section 3: Divider, Instructions, and Accordion Menu
            <div class="px-4 lg:pb-4 lg:px-0">
                <hr class="border border-solid border-gray-400 mx-auto w-full mb-6"/>
                <h2 class="text-center pb-4 text-[1.5rem] text-[#f7931a] font-semibold">"Start Here"</h2>
                <AccordionMenu faq_name="node_setup".to_string()/>
            </div>
        </div>
    }
}
