use leptos::*;

/// Renders the Blog page of the application.
#[component]
pub fn BlogPage() -> impl IntoView {
    view! {
       <div id="about" class="grid gap-6 max-w-5xl mx-auto pb-20 animate-fadeinone grid-rows-[auto_auto_1fr] lg:gap-8">
            // Section 1: Title and Subtitle
            <div class="flex flex-col mx-auto px-4 pt-10 lg:pt-0">
                <h1 class="text-center text-[2.25rem] text-[#f7931a] font-semibold font-title leading-tight lg:text-[4rem]">"Posts"</h1>
                <div class="text-center max-w-sm mx-auto pt-4">
                    <p class="text-lg text-white">"Coming...soon ™"</p>
                </div>
            </div>

            // Section 2: Under Construction Image
            <div class="px-4 lg:pt-0 lg:px-0 flex justify-center">
                <img src="./../../../writing.jpg" alt="Under construction image" class="max-w-full h-auto rounded-md"/>
            </div>
        </div>
    }
}

// Setup postgress DB and add SQLx to interact with it.
// Setup API - create_post, fetch_post
// Create a template that renders fetched post into view.

//
//pub struct Post {
//    id: u32,
//    title: String,
//    introduction: String,
//    images: Vec<String>,
//    section_one: String,
//    section_two: String,
//    section_three: String,
//    section_four: String,
//    section_five: String,
//    section_six: String,
//    conclusion: String,
//}

//pub struct Post {
//    pub title: String,
//    pub slug: String,
//    pub excerpt: Option<String>,
//    pub content: String,
//    pub toc: Option<String>,
//    pub created_at: DateTime<Utc>,
//    pub updated_at: DateTime<Utc>,
//    pub published: bool,
//    pub preview: bool,
//    pub links: Option<String>,
//    pub tags: Vec<String>,
//}
