use leptos::*;

/// Renders the Blog page of the application.
#[component]
pub fn BlogPage() -> impl IntoView {
    view! {
        <div id="about" class="flex flex-col max-w-3xl mx-auto rounded-xl pb-10 animate-fadeinone">
            <div class="flex flex-col p-6 pt-10 max-w-3xl mx-auto">
                    <h1 class="flex justify-center text-[36px] text-white font-semibold">"Posts"</h1>
                <div class="flex justify-center pt-4 max-w-sm">
                    <p class="text-sm text-white">"Random thoughts about bitcoin and stuff."</p>
                </div>
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
