use leptos::{*, ev::MouseEvent};

#[derive(Debug, Clone)]
pub struct FAQS {
    pub title: String,
    pub p_1: Vec<String>,
    pub a_1: Vec<String>,
}

#[component]
fn Menu<F>(cx: Scope, faq_title: String, on_click: F, #[prop(optional)] markdown: String) -> impl IntoView
    where
        F: Fn(MouseEvent) + 'static,
    {       
    let source = markdown::to_html(
        "
You'll want to locate the **12 words** and **passphrase** you wrote down when setting up your wallet. Either re-install Samourai and follow their restore guide [here]( https://docs.samourai.io/wallet/restore-recovery) or use any other BIP39 compatible wallet.  


>I highly recommend that once the wallet is recovered that you transfer all funds to a new wallet. Although your Samourai backup is encrypted on your phone, it's best to er on the side of caution, you should consider the original wallet compromised. 
");


    log!("Source: {} ", source );

    view! {cx,
        <h2 id="accordion-collapse-heading-1" on>
            <button type="button" class="flex items-center justify-between w-full p-5 font-medium
            text-left text-gray-900 border border-b-0 border-gray-700 rounded-t-xl focus:ring-2
            focus:ring-gray-200 hover:bg-[#3c6594]"
            data-accordion-target="#accordion-collapse-body-1" aria-expanded="true"
            aria-controls="accordion-collapse-body-1">
                <span inner_html=faq_title/> 
                <svg data-accordion-icon class="w-3 h-3 rotate-180 shrink-0" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 10 6">
                    <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5 5 1 1 5"/>
                </svg>
            </button>
        </h2>
        <div id="accordion-collapse-body-1"  aria-labelledby="accordion-collapse-heading-1">
            <div class="p-5 border border-b-0 border-gray-700 hidden">
                <div class="bg-[#3c6594] rounded-md p-2.5" inner_html=source/>
            </div>
        </div>
    }
}

#[component]
pub fn AccordionMenu(cx: Scope, faqs: Vec<FAQS>) -> impl IntoView {

    let faqs = faqs.iter();
    let markdown = "Poop";
    

    let on_click = "poop".to_string(); 

    view! {cx, 
       <div id="accordion-collapse" data-accordion="collapse">
           <Menu faq_title=markdown::to_html("## **How do I create a wallet?**") on_click=on_click /> 
       </div>
           
    }
}
