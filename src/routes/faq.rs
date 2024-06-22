use crate::extras::accordion_menu::AccordionMenu;
use leptos::*;

/// Renders the faq page of the application.
#[component]
pub fn FaqPage() -> impl IntoView {
    view! {
        <div id="about" class="flex flex-col max-w-5xl h-screen mx-auto p-6 md:pt-10 animate-fadeinone">
                <h1 class="flex text-[4.5rem] text-white font-semibold text-center justify-center">"The Bitcoin Help Desk"</h1>
                <div class="text-center mt-3 md:mt-0 italic max-w-3xl mx-auto">
                    <p class="text-white text-lg mx-10 pb-10">
                        "Controlling a bitcoin private key grants absolute authority over the
                        associated bitcoin, embodying the ethos of the bitcoin movement. Self custody and personal
                        responsibility restore power and sovereignty, eliminating reliance on third parties,
                        particularly the state."
                    </p>
                </div>
                <div class="flex flex-col items-center w-full py-4">
                    <h2 class="text-xl text-white py-2">"Commonly asked questions:"</h2>
                </div>
                    <AccordionMenu faq_name="general".to_string()/>
                <div class="flex flex-col items-center pt-6">
                    <p class="text-sm text-white">"Need additional help? Reach out to me by email: "<a class="underline text-[#678096] hover:text-[#3c6594]" href="mailto:Bayernator@protonmail.com" target="_blank" rel="noopener noreferrer">Bayernator@protonmail.com</a></p>
                    <br />
                    <p class="text-sm text-white">"Or connect via "<a class="underline text-[#678096] hover:text-[#3c6594]" href="https://github.com/simplex-chat" target="_blank" rel="noopener noreferrer">Simplex Chat</a>" by scanning the QR code:"</p>
                    <br />
                    <div class="">
                        <img src="./../../../simplexqr.png" alt="simplex_qr_code" width="150" height="150"/>
                    </div>
                </div>
        </div>
    }
}
