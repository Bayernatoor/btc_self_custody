use leptos::*;

#[allow(clippy::redundant_closure)]
#[component]
pub fn Footer() -> impl IntoView {
    view! {
        <footer class="flex flex-col items-center text-white text-center text-surface lg:text-left">
            <div class="w-full pb-8">
                <hr class="mx-auto border border-solid border-white"/>
            </div>
            <div class="max-w-5xl">
                // left side text
                <div class="grid gap-4 mx-auto lg:grid-cols-2 lg:p-4">
                    <div class="lg:mb-0 lg:mr-56">
                        <h5 class="mb-2 font-semibold uppercase">"WE HODL BTC"</h5>

                        <p class="mx-4 lg:p-0 lg:mx-0 lg:mt-2">
                            "WE HODL BTC is a free resource created to help bitcoiners take
                            self-custody of their bitcoin. Whether it's 100 satoshis or 100 bitcoins, there's a guide for you."
                        </p>

                        <p class="my-2">"Knowledge is Freedom"</p>
                    </div>

                    // right side social links
                    <div class="my-6 lg:my-0">
                        <div class="flex flex-row justify-center space-x-2 lg:justify-end">
                            // Github link
                            <a
                                href="https://github.com/Bayernatoor/btc_self_custody"
                                target="_blank"
                                rel="noopener noreferrer"
                                type="button"
                                class="rounded-full bg-[#333333] p-3 uppercase leading-normal text-white transition duration-150 ease-in-out hover:text-orange-400 focus:shadow-dark-1 focus:outline-none focus:ring-0 active:shadow-1 dark:text-white"
                                data-twe-ripple-init
                                data-twe-ripple-color="light"
                            >
                                <span class="mx-auto [&>svg]:h-10 [&>svg]:w-10">
                                    <svg
                                        xmlns="http://www.w3.org/2000/svg"
                                        fill="currentColor"
                                        viewBox="0 0 496 512"
                                    >
                                        class=
                                        "fill-current hover:text-orange-500 dark:text-gray-200 dark:hover:text-orange-400"
                                        >
                                        <path d="M165.9 397.4c0 2-2.3 3.6-5.2 3.6-3.3 .3-5.6-1.3-5.6-3.6 0-2 2.3-3.6 5.2-3.6 3-.3 5.6 1.3 5.6 3.6zm-31.1-4.5c-.7 2 1.3 4.3 4.3 4.9 2.6 1 5.6 0 6.2-2s-1.3-4.3-4.3-5.2c-2.6-.7-5.5 .3-6.2 2.3zm44.2-1.7c-2.9 .7-4.9 2.6-4.6 4.9 .3 2 2.9 3.3 5.9 2.6 2.9-.7 4.9-2.6 4.6-4.6-.3-1.9-3-3.2-5.9-2.9zM244.8 8C106.1 8 0 113.3 0 252c0 110.9 69.8 205.8 169.5 239.2 12.8 2.3 17.3-5.6 17.3-12.1 0-6.2-.3-40.4-.3-61.4 0 0-70 15-84.7-29.8 0 0-11.4-29.1-27.8-36.6 0 0-22.9-15.7 1.6-15.4 0 0 24.9 2 38.6 25.8 21.9 38.6 58.6 27.5 72.9 20.9 2.3-16 8.8-27.1 16-33.7-55.9-6.2-112.3-14.3-112.3-110.5 0-27.5 7.6-41.3 23.6-58.9-2.6-6.5-11.1-33.3 2.6-67.9 20.9-6.5 69 27 69 27 20-5.6 41.5-8.5 62.8-8.5s42.8 2.9 62.8 8.5c0 0 48.1-33.6 69-27 13.7 34.7 5.2 61.4 2.6 67.9 16 17.7 25.8 31.5 25.8 58.9 0 96.5-58.9 104.2-114.8 110.5 9.2 7.9 17 22.9 17 46.4 0 33.7-.3 75.4-.3 83.6 0 6.5 4.6 14.4 17.3 12.1C428.2 457.8 496 362.9 496 252 496 113.3 383.5 8 244.8 8zM97.2 352.9c-1.3 1-1 3.3 .7 5.2 1.6 1.6 3.9 2.3 5.2 1 1.3-1 1-3.3-.7-5.2-1.6-1.6-3.9-2.3-5.2-1zm-10.8-8.1c-.7 1.3 .3 2.9 2.3 3.9 1.6 1 3.6 .7 4.3-.7 .7-1.3-.3-2.9-2.3-3.9-2-.6-3.6-.3-4.3 .7zm32.4 35.6c-1.6 1.3-1 4.3 1.3 6.2 2.3 2.3 5.2 2.6 6.5 1 1.3-1.3 .7-4.3-1.3-6.2-2.2-2.3-5.2-2.6-6.5-1zm-11.4-14.7c-1.6 1-1.6 3.6 0 5.9 1.6 2.3 4.3 3.3 5.6 2.3 1.6-1.3 1.6-3.9 0-6.2-1.4-2.3-4-3.3-5.6-2z"></path>
                                    </svg>
                                </span>
                            </a>

                            // NOSTR link
                            <a
                                href="https://nostr.com/npub1hxcjalw99u4m7vcalnrrgkdvyqftglydrt6tm2q9afnvec55guysrwkq9z"
                                target="_blank"
                                rel="noopener noreferrer"
                                type="button"
                                class="rounded-full bg-[#333333] p-3 uppercase leading-normal text-white transition duration-150 ease-in-out hover:shadow-dark-1 focus:shadow-dark-1 focus:outline-none focus:ring-0 active:shadow-1 dark:text-white"
                                data-twe-ripple-init
                                data-twe-ripple-color="light"
                            >
                                <span class="mx-auto [&>svg]:h-10 [&>svg]:w-10">
                                    <svg
                                        xmlns="http://www.w3.org/2000/svg"
                                        viewBox="0 0 512 512"
                                        class="fill-current hover:text-orange-500 dark:text-gray-200 dark:hover:text-orange-400"
                                    >
                                        <path d="M278.5 215.6 23 471c-9.4 9.4-9.4 24.6 0 33.9s24.6 9.4 33.9 0l57-57h68c49.7 0 97.9-14.4 139-41 11.1-7.2 5.5-23-7.8-23-5.1 0-9.2-4.1-9.2-9.2 0-4.1 2.7-7.6 6.5-8.8l81-24.3c2.5-.8 4.8-2.1 6.7-4l22.4-22.4c10.1-10.1 2.9-27.3-11.3-27.3H377c-5.1 0-9.2-4.1-9.2-9.2 0-4.1 2.7-7.6 6.5-8.8l112-33.6c4-1.2 7.4-3.9 9.3-7.7 10.8-21 16.4-44.5 16.4-68.6 0-41-16.3-80.3-45.3-109.3l-5.5-5.5C432.3 16.3 393 0 352 0s-80.3 16.3-109.3 45.3L139 149c-48 48-75 113.1-75 181v55.3l189.6-189.5c6.2-6.2 16.4-6.2 22.6 0 5.4 5.4 6.1 13.6 2.2 19.8z"></path>
                                    </svg>
                                </span>
                            </a>

                            // Email Link
                            <a
                                href="mailto:wehodlbtc@pm.me"
                                target="_blank"
                                rel="noopener noreferrer"
                                type="button"
                                class="rounded-full bg-[#333333] p-3 uppercase leading-normal text-white transition duration-150 ease-in-out hover:shadow-dark-1 focus:shadow-dark-1 focus:outline-none focus:ring-0 active:shadow-1 dark:text-white"
                                data-twe-ripple-init
                                data-twe-ripple-color="light"
                            >
                                <span class="mx-auto [&>svg]:h-10 [&>svg]:w-10">
                                    <svg
                                        xmlns="http://www.w3.org/2000/svg"
                                        width="64"
                                        height="64"
                                        viewBox="-0.5 -0.5 64 64"
                                        class="fill-current hover:text-orange-500 dark:text-gray-200 dark:hover:text-orange-400"
                                    >
                                        <path d="M31.5-.45S12.646-1.07 8.757 19.845V34.1s.16 1.525 4.45 4.62S28.594 50.526 31.5 50.526s14.007-8.718 18.294-11.815 4.45-4.62 4.45-4.62V19.845C50.354-1.07 31.5-.45 31.5-.45zM44.4 27.6H18.6v-7.766C21.2 9.46 31.5 9.316 31.5 9.316s10.3.143 12.9 10.53zM31.5 53.734s-2.922-.286-5.2-1.874L8.757 39.378v21.566s.13 2.5 2.892 2.5h39.7c2.763 0 2.892-2.5 2.892-2.5V39.378L36.7 51.86c-2.287 1.588-5.2 1.874-5.2 1.874z"></path>
                                    </svg>
                                </span>
                            </a>
                        </div>
                    </div>
                </div>
            </div>
            // made by portion
            <div class="w-full pt-2 pb-6 text-center text-[#f7931a]">
                "2024 "
                <a
                    class="underline"
                    href="https://github.com/Bayernatoor"
                    target="_blank"
                    rel="noopener noreferrer"
                >
                    "Made by Bayer"
                </a>
            </div>
        </footer>
    }
}
