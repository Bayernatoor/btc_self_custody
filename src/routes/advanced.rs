use crate::extras::accordion_menu::AccordionMenu;
use leptos::*;

#[component]
#[allow(non_snake_case)]
pub fn AdvancedPage() -> impl IntoView {
    let title = "Advanced Self-Custody Guide".to_string();
    let quote = "".to_string();
    let quote_author = "".to_string();
    let explainer  = "Taking self-custody of your bitcoin comes with great responsibility, especially when that bitcoin could become generational wealth, therefore it is
        wise to take extra precautions. That being said, we should take care to keep things as simple as possible, while also ensuring a high degree of privacy and security.
        A secure and private advanced self-custody setup looks like the following: ".to_string();

    view! {
        <div id="basic" class="flex flex-col max-w-4xl mx-auto rounded-xl p-4 animate-fadeinone md:transform md:scale-125 md:pt-28">
            <div class="flex flex-col p-6 pt-10 max-w-3xl mx-auto">
                    <h1 class="flex justify-center text-[36px] text-white font-semibold">{title}</h1>
            </div>

            <p class="font-bold text-lg text-white">"MultiSignature Wallet"</p>
            <p class="pb-2 text-white">""</p>
            //<p class="mr-4 text-md text-white">
            //    "You've got quite the stash to protect now. This Advanced guide builds on-top of our previous basic/intermediate desktop guide."
            //    <a class="text-[#8cb4ff] underline-offset-auto" href="/guides/basic/desktop">" basic desktop guide "</a>
            //    "before continuing."
            //</p>
            <p class="py-2 text-md text-white">{explainer}</p>
                <ol class="list-decimal pl-4 pt-2 text-md leading-normal text-white">
                    <li>"Setup and run your own Bitcoin node"</li>
                    <li>"Setup a 2 of 3 Multisig in Sparrow Wallet using 3 signing devices"</li>
                    <li>"Use Sparrow Wallet to coordinate the Multisig. Preferably on a dedicated computer"</li>
                    <li>"Backup your seed words and passphrases on steel"</li>
                    <li>"Safely backup and store your Multisig Wallet's Output Descriptors"</li>
                    <li>"Store the backups and devices in different geographic locations"</li>
                </ol>
            <p class="italic pt-4 text-md text-white">
               "Before starting, I encourage you to read through all the steps below, so as to get an understanding of the options available to you.
               The advanced section is optional, however, if you decide to implement certain parts, you should to do it from the start."
            </p>
           // <a class="text-[#8cb4ff] underline-offset-auto" href="/guides/basic/desktop">" basic desktop guide "</a>
            <div class="mx-auto max-w-xl p-4 w-full" >
                <div class="mx-auto border border-solid border-gray-400"></div>
            </div>

            <h2 class="flex justify-center font-bold text-xl text-white pt-6 pb-2">"Advanced Setup"</h2>
            <AccordionMenu faq_name="advanced_desktop_setup".to_string()/>
        </div>

    }
}
