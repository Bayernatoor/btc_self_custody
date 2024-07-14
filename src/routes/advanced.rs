use crate::extras::accordion_menu::AccordionMenu;
use leptos::*;

// Renders the Advanced guide page
#[component]
#[allow(non_snake_case)]
pub fn AdvancedPage() -> impl IntoView {
    let title = "Advanced Self-Custody Guide".to_string();
    let _quote = "".to_string();
    let _quote_author = "".to_string();
    let explainer  = "Taking self-custody of your bitcoin comes with great responsibility, especially when that bitcoin could become generational wealth, therefore it is
        wise to take extra precautions. That being said, we should take care to keep things as simple as possible, while also ensuring a high degree of privacy and security.
        A secure and private advanced self-custody setup looks like the following: ".to_string();

    view! {
        <div
            id="advanced"
            class="grid gap-6 max-w-3xl mt-8 mb-24 mx-auto animate-fadeinone grid-rows-[auto_auto_1fr] md:max-w-4xl lg:max-w-5xl lg:gap-8 md:my-28"
        >
            // Section 1: Title
            <div class="flex flex-col mx-auto px-4">
                <h1 class="text-center text-[2.25rem] text-[#f7931a] font-title font-semibold md:text-[2.5rem] lg:text-[3rem]">
                    {title}
                </h1>
            </div>

            // Section 2: Intro and Steps
            <div class="px-4 lg:pt-0 lg:px-0">
                <h2 class="text-left text-[1.5rem] text-[#f7931a] font-semibold">
                    "MultiSignature Wallet"
                </h2>
                <p class="py-2 text-lg text-white">{explainer}</p>
                <ol class="list-decimal pl-8 pt-2 text-lg leading-normal text-white">
                    <li>"Setup and run your own Bitcoin node"</li>
                    <li>"Setup a 2 of 3 Multisig in Sparrow Wallet using 3 signing devices"</li>
                    <li>
                        "Use Sparrow Wallet to coordinate the Multisig. Preferably on a dedicated computer"
                    </li>
                    <li>"Backup your Seed Words and Passphrases on steel"</li>
                    <li>"Safely backup and store your Multisig Wallet's Output Descriptors"</li>
                    <li>"Store the backups and devices in different geographic locations"</li>
                </ol>
                <p class="italic pt-4 text-lg text-white">
                    "Before starting, I encourage you to read through all the steps below, so as to get an understanding of the options available to you.
                    The advanced section is optional, however, if you decide to implement certain parts, you'll want to be aware of them before starting."
                </p>
            </div>

            // Section 3: Additional Content and Accordion Menu
            <div class="px-4 lg:pb-4 lg:px-0">
                <hr class="border border-solid border-gray-400 mx-auto w-full mb-6"/>
                <h3 class="py-4 text-center text-[1.5rem] text-[#f7931a] font-semibold">
                    "Advanced Setup"
                </h3>
                <AccordionMenu faq_name="advanced_desktop_setup".to_string()/>
            </div>
        </div>
    }
}
