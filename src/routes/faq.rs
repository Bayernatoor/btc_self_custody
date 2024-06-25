use crate::extras::accordion_menu::AccordionMenu;
use leptos::*;

/// Renders the faq page of the application.
#[component]
pub fn FaqPage() -> impl IntoView {
    view! {
        <div id="about" class="grid gap-6 max-w-5xl mx-auto p-6 md:pt-10 animate-fadeinone grid-rows-[auto_auto_1fr] lg:gap-8">
            // Section 1: Title and Intro
            <div class="mt-10 lg:mt-0">
                <h1 class="text-center text-[2.25rem] text-[#f7931a] font-semibold justify-center leading-tight font-title lg:p-6 lg:text-[4rem]">"The Bitcoin Help Desk"</h1>
                <div class="text-center mt-4 md:mt-0 italic max-w-3xl mx-auto">
                    <p class="text-white text-lg pb-10">
                        "Controlling a bitcoin private key grants absolute control over the
                        associated bitcoin, embodying the ethos of the bitcoin movement. Self custody and personal
                        responsibility restore power and sovereignty, eliminating reliance on third parties,
                        particularly the state."
                    </p>
                </div>
            </div>

            // Section 2: Commonly Asked Questions
            <div class="pt-4 lg:pt-0 lg:px-0">
                <div class="flex flex-col items-center w-full pb-4">
                    <h2 class="text-xl text-[#f7931a] text-center font-semibold md:text-2xl">"Commonly asked questions:"</h2>
                </div>
                <AccordionMenu faq_name="general".to_string()/>
            </div>

            // Section 3: Contact Information
            <div class="pb-6">
                <hr class="border border-solid border-gray-400 mx-auto w-full mb-6"/>

                <div class="flex flex-col items-center pt-6">
                    <p class="text-lg text-white">"Need additional help? Reach out to me by email: "<a class="underline text-[#8cb4ff] hover:text-[#3c6594]" href="mailto:bayernator@protonmail.com" target="_blank" rel="noopener noreferrer">bayernator@protonmail.com</a></p>
                    <br />
                    <p class="text-lg text-white">"Or connect via "<a class="underline text-[#8cb4ff] hover:text-[#3c6594]" href="https://github.com/simplex-chat" target="_blank" rel="noopener noreferrer">Simplex Chat</a>" by scanning the QR code:"</p>
                    <br />
                    <div>
                        <img src="./../../../simplexqr.png" alt="simplex_qr_code" width="150" height="150"/>
                    </div>
                </div>
            </div>
    </div>
    }
}
