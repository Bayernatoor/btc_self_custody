use leptos::prelude::*;
use leptos_meta::*;

/// Renders the About page — card-based layout.
#[component]
pub fn AboutPage() -> impl IntoView {
    view! {
        <Title text="About | WE HODL BTC"/>

        <div class="grid gap-8 mx-auto max-w-3xl mt-8 mb-24 opacity-0 animate-fadeinone grid-rows-[auto_auto_1fr] md:max-w-4xl lg:max-w-5xl md:my-24">
            // Header Section
            <header class="flex flex-col mx-auto px-4 pt-10">
                <h1 class="text-[2rem] text-[#f7931a] text-center font-semibold leading-tight font-title pb-2 md:text-[2.5rem] lg:text-[3rem]">
                    "WE HODL BTC"
                </h1>
                <hr class="mx-auto border-2 border-solid border-white w-1/2 lg:w-full"/>
                <p class="text-sm lg:text-base text-center text-white px-6 pt-4 leading-relaxed">
                    "We Hodl BTC is about taking self-custody of your bitcoin.
                    The goal is to help anyone take ownership regardless of how many satoshis or bitcoin they own.
                    Bitcoin's core values are of self-sovereignty, decentralization, trust minimization and preservation of wealth.
                    I believe that taking self-custody of your bitcoin naturally follows."
                </p>
            </header>

            // Main Content — Cards
            <main class="px-4 lg:pt-0 lg:px-0 grid gap-6 lg:gap-4 lg:grid-cols-3">
                // About Card
                <div class="flex flex-col px-5 py-4 mx-auto max-w-xs bg-[#1a4a72] rounded-xl text-left text-white leading-relaxed">
                    <h2 class="text-lg lg:text-xl text-[#f7931a] font-semibold">"About"</h2>
                    <p class="text-sm pt-2">
                        "I go by Bayer, I am a Bitcoiner who believes bitcoin is the most significant discovery of our time.
                        In a world conditioned to spend endlessly, bitcoin rewards those who embrace saving, fostering a mindset
                        of low time preference and incentivizing long-term thinking and planning."
                    </p>
                    <p class="text-sm pt-2">
                        <strong>"PGP Key: "</strong>
                        <a
                            class="underline text-blue-400 hover:text-[#3c6594]"
                            href="./../../../public_key.asc"
                            target="_blank"
                            rel="noopener noreferrer"
                        >
                            "Download"
                        </a>
                    </p>
                    <p class="text-sm pt-2">
                        <strong>"Find me on: "</strong>
                        <a
                            class="underline text-blue-400 hover:text-[#3c6594]"
                            href="https://primal.net/bayer"
                            target="_blank"
                            rel="noopener noreferrer"
                        >
                            "Nostr"
                        </a>
                        " | "
                        <a
                            class="underline text-blue-400 hover:text-[#3c6594]"
                            href="https://github.com/Bayernatoor"
                            target="_blank"
                            rel="noopener noreferrer"
                        >
                            "Github"
                        </a>
                    </p>
                </div>

                // Donate Card
                <div class="flex flex-col px-5 py-4 mx-auto max-w-xs bg-[#1a4a72] rounded-xl text-left text-white leading-relaxed">
                    <h2 class="text-lg lg:text-xl text-[#f7931a] font-semibold">"Donate"</h2>
                    <p class="text-sm pt-2">
                        "Your contributions help keep the project running and are greatly appreciated. If you want to support me you can do so below. Thank you!"
                    </p>
                    <p class="text-sm pt-2">
                        <strong>"Lightning Address:"</strong>
                    </p>
                    <p class="text-sm">
                        <a
                            class="underline text-blue-400 hover:text-[#3c6594]"
                            href="lightning:bayer@primal.net"
                            target="_blank"
                            rel="noopener noreferrer"
                        >
                            " bayer@primal.net"
                        </a>
                    </p>

                    <p class="text-sm pt-2 break-words">
                        <strong>"PayNym (BIP47):"</strong>
                    </p>
                    <p class="text-sm">
                        <a
                            class="underline text-blue-400 hover:text-[#3c6594] break-words"
                            href="https://paynym.is/+wildhaze2Ff"
                            target="_blank"
                            rel="noopener noreferrer"
                        >
                            " +wildhaze2Ff"
                        </a>
                    </p>

                    <p class="text-sm pt-2 break-words">
                        <strong>"On-chain:"</strong>
                        <a
                            class="underline text-blue-400 hover:text-[#3c6594] break-words"
                            href="bitcoin:bc1pg3l4kqvurd3w350mgr4amcplj7ar70gqyck9hzfu75w5ylrvl3rst84h3d"
                            target="_blank"
                            rel="noopener noreferrer"
                        >
                            " bc1pg3l4kq...st84h3d"
                        </a>
                        <img
                            class="h-auto w-[120px] pt-2"
                            src="./../../../bitcoin_donation_address_qr.png"
                            alt="on-chain bitcoin address qr code"
                        />
                    </p>
                </div>

                // Contribute Card
                <div class="flex flex-col px-5 py-4 mx-auto max-w-xs bg-[#1a4a72] rounded-xl text-left text-white leading-relaxed">
                    <h2 class="text-lg lg:text-xl text-[#f7931a] font-semibold">"Contribute"</h2>
                    <p class="text-sm pt-2">
                        "This project was developed with the purpose of helping anyone learn about bitcoin self-custody, as well as a means for me to learn Rust and web development.
                        It is entirely free and open-sourced under an MIT license. Contributions are always welcome, please feel free to open an "
                        <a
                            class="underline text-blue-400 hover:text-[#3c6594]"
                            href="https://github.com/Bayernatoor/btc_self_custody"
                            target="_blank"
                            rel="noopener noreferrer"
                        >
                            "issue on Github."
                        </a>
                    </p>
                    <p class="text-sm pt-2">
                        <strong>"Questions?"</strong>
                        " Reach out via Nostr or send me an "
                        <a
                            class="underline text-blue-400 hover:text-[#3c6594]"
                            href="mailto:wehodlbtc@pm.me"
                            target="_blank"
                            rel="noopener noreferrer"
                        >
                            "email."
                        </a>
                    </p>
                </div>
            </main>
        </div>
    }
}
