use crate::extras::accordion_menu::AccordionMenu;
use leptos::*;

/// Renders the faq page of the application.
#[component]
pub fn FaqPage() -> impl IntoView {
    let quote = "Free software is a matter of liberty, not price. To understand the concept, you should think of 'free' as in 'free speech,' not as in 'free beer'." ;
    let quote_author = "- Richard Stallman";
    let intro =  "The guides are meant to help anyone self-custody their bitcoin, regardless of how much they own. The guides are opinionated, recommending few options to
                 help cut through the noise and streamline the self-custody process. All software recommendations are licensed using open and permissive licenses which follow 
                 the principales set forth by Richard Stallman regarding Free Software. The intention is to eliminate the need to trust me, since you
                 can verify it for yourself.";

    view! {
        <div
            id="about"
            class="grid gap-6 max-w-3xl mx-auto p-6 mt-8 mb-24 animate-fadeinone grid-rows-[auto_auto_1fr] md:max-w-4xl lg:max-w-5xl lg:gap-8"
        >
            // Section 1: Title and Intro
            <div class="">
                <h1 class="text-center text-[2.25rem] text-[#f7931a] font-semibold justify-center leading-tight font-title md:p-4 md:text-[3rem] lg:text-[4rem]">
                    "The Bitcoin Help Desk"
                </h1>
                <div class="text-center mt-4 md:mt-0 max-w-3xl mx-auto md:max-w-4xl lg:max-w-4xl">
                    <div class="text-center mx-auto">
                        <p class="text-lg text-white italic">{quote}</p>
                    </div>
                    <div class="text-center mx-auto">
                        <p class="text-sm text-white italic">{quote_author}</p>
                    </div>
                    <div class="text-center mt-4 mx-auto">
                        <p class="text-lg text-white">{intro}</p>
                    </div>
                </div>
            </div>

            // Section 2: Commonly Asked Questions
            <div class="pt-4 lg:pt-0 lg:px-0">
                <div class="flex flex-col items-center w-full pb-4">
                    <h2 class="text-xl text-[#f7931a] text-center font-semibold md:text-2xl">
                        "Commonly asked questions:"
                    </h2>
                </div>
                <AccordionMenu faq_name="general".to_string()/>
            </div>

            // Section 3: Contact Information
            <div class="pb-6">
                <hr class="border border-solid border-gray-400 mx-auto w-full mb-6"/>

                <div class="flex flex-col items-center text-center pt-6">
                    <p class="text-lg text-white">
                        "Need additional help? Reach out to me by email: "
                        <a
                            class="underline text-[#8cb4ff] hover:text-[#3c6594]"
                            href="mailto:wehodlbtc@pm.me"
                            target="_blank"
                            rel="noopener noreferrer"
                        >
                            wehodlbtc@pm.me
                        </a>
                    </p>
                    <br/>
                    <p class="text-lg text-white">
                        "Or connect via "
                        <a
                            class="underline text-[#8cb4ff] hover:text-[#3c6594]"
                            href="https://github.com/simplex-chat"
                            target="_blank"
                            rel="noopener noreferrer"
                        >
                            Simplex Chat
                        </a> " by scanning the QR code:"
                    </p>
                    <br/>
                    <div>
                        <img
                            src="./../../../simplexqr.png"
                            alt="simplex_qr_code"
                            width="150"
                            height="150"
                        />
                    </div>
                </div>
            </div>
        </div>
    }
}
