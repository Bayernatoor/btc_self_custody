use leptos::*;

/// Renders the faq page of the application.
#[component]
pub fn FaqPage() -> impl IntoView {
    view! {
        <div id="about" class="flex flex-col items-center max-w-3xl mx-auto rounded-xl pb-10 animate-fadein">
            <div class="flex flex-col items-center p-6 pt-10 max-w-3xl mx-auto">
                    <h1 class="flex text-[36px] text-white font-semibold">"The Bitcoin Barrack Help Desk"</h1>
                <div class="flex flex-col items-center pt-4"> 
                    <p class="text-sm text-white">"Need help? Reach out to me by email: "<a class="underline text-[#678096] hover:text-[#3c6594]" href="mailto:Bayernator@protonmail.com" target="_blank" rel="noopener noreferrer">Bayernator@protonmail.com</a></p>
                    <br />
                    <p class="text-sm text-white">"Or connect via "<a class="underline text-[#678096] hover:text-[#3c6594]" href="https://github.com/simplex-chat" target="_blank" rel="noopener noreferrer">Simplex Chat</a>" by scanning the QR code:"</p>
                    <br />
                    <div class="">
                        <img class="" src="./../../../simplexqr.png" alt="simplex_qr_code"/>
                    </div>
                </div>
            </div>
        </div>
    }

}
