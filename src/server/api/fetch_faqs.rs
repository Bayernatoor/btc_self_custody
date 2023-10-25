use leptos::*;
use std::fs;
use leptos::logging::log;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FAQ {
    pub id: u32,
    pub title: String,
    pub content: String,
}

impl FAQ {
    pub fn new_faq(id: u32, title: String, content: String) -> Self {
        Self { id, title, content }
    }
}
#[server(FetchFaq, "/api")]
pub async fn fetch_faq(wallet: String) -> Result<Vec<FAQ>, ServerFnError> {
    let path = format!("./src/faqs/{}", wallet);

    let faqs_dir = fs::read_dir(path)?;

    let mut faqs = Vec::new();
    let mut id = 0;
    //let content = fs::read_to_string("./src/faqs/samourai/samourai_faq2.md")?;
    //let title = &content.split("\n").collect::<Vec<&str>>()[0].to_string();
    //let faq_content = &content.split("\n").collect::<Vec<&str>>()[1..].join("\n");
    //log!("Content: {:?}",faq_content);
    //faqs.push(FAQ::new_faq(1, title.to_string(), faq_content.to_string()));

    for faq in faqs_dir {
        let faq = faq?;
        //log!("files in dir:  {:?}", faq);
        let path = faq.path();
        //log!("path buffer:  {:?}", path);
        // increment id for each new file
        id += 1;
        // get name of file
        let _file_name = path.file_name().unwrap().to_str().unwrap();

        // read contents of file
        let content = fs::read_to_string(path)?;

        // get the faq title
        let title = &content.split("\n").collect::<Vec<&str>>()[0].to_string();

        // get faq content
        let faq_content = &content.split("\n").collect::<Vec<&str>>()[1..].join("\n");

        //log!("faq_content: {:?}", faq_content);
        //log!("content: {:?}", content);


        faqs.push(FAQ::new_faq(id, title.to_string(), faq_content.to_string()));
    }

    //log!("FetchFaq Returns: {:?}", faqs);
    Ok(faqs)
}
