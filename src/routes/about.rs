use leptos::*;

/// Renders the About page of the application.
#[component]
pub fn AboutPage() -> impl IntoView {
    view! {
        <div class="grid gap-10 mx-auto max-w-5xl my-20 opacity-0 animate-fadeinone lg:my-24">
            // Header Section
            <header class="flex flex-col text-center text-white leading-loose mx-auto max-w-5xl p-4">
                <h1 class="text-3xl text-center pb-2 lg:text-5xl font-semibold">"We Hodl BTC"</h1>
                <hr class="mx-auto border-2 border-solid border-gray-400 w-1/2 lg:w-full"/>
                <p class="text-md lg:text-lg pt-2 lg:text-left">"We Hodl BTC is about taking self-custody of your bitcoin. The goal is to help everyone take ownership regardless of how many satoshis or bitcoin they own. Bitcoin’s core values are of self resilience, decentralization, trust minimization and preservation of wealth. I believe that taking self-custody of your bitcoin naturally follows."</p>
            </header>

            // Main Content
            <main class="grid gap-10 lg:gap-6 lg:grid-cols-3">
                // Me Section
                <div class="flex flex-col p-6 mx-auto text-center text-white leading-loose lg:text-left">
                    <h1 class="text-2xl lg:text-3xl font-semibold">"Me"</h1>
                    <p class="text-md pt-2"><a href="https://github.com/Bayernatoor" class="underline text-blue-400">"Github"</a></p>
                    <p class="text-md pt-2">"Nostr: ..."</p>
                    <p class="text-md pt-2">"I go by Bayer, I am a Bitcoiner and believe that bitcoin is the greatest discovery of our millennia, the best savings mechanism there is and hope ..."</p>
                </div>
                // Contribute Section
                <div class="flex flex-col p-6 mx-auto text-center text-white leading-loose lg:text-left">
                    <h1 class="text-2xl lg:text-3xl font-semibold">"Contribute"</h1>
                    <p class="text-md pt-2">"This project was developed with the purpose of helping others learn about bitcoin self-custody as well as for me to learn to code. It is entirely free and open-source under an MIT license. If you would like to contribute please feel free to open an issue."</p>
                    <p class="text-md pt-2">"Questions? Hop into the discord channel."</p>
                </div>
                // Donate Section
                <div class="flex flex-col p-6 mx-auto text-center text-white leading-loose lg:text-left">
                    <h1 class="text-2xl lg:text-3xl font-semibold">"Donate"</h1>
                    <p class="text-md pt-2">"Your contributions help keep the project running and are greatly appreciated. Please consider supporting us through donations."</p>
                </div>
            </main>
        </div>


    }
}
