use leptos::{ServerFnError, server};
use serde::{Deserialize, Serialize};
#[allow(unused_imports)]
use std::{fs, io};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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
pub async fn fetch_faq(faq_name: String) -> Result<Vec<FAQ>, ServerFnError> {
    let path = format!("./src/faqs/{}", faq_name);

    let mut files = fs::read_dir(path)?
        .map(|dir| dir.map(|file| file.path()))
        .collect::<Result<Vec<_>, io::Error>>()?;

    files.sort();

    let mut faqs = Vec::new();
    let mut id = 0;

    for faq in files {
        // increment id for each new file
        id += 1;
        // get name of file
        let _file_name = faq.file_name().unwrap().to_str().unwrap();

        // read contents of file
        let content = fs::read_to_string(faq)?;

        // get the faq title
        let title = &content.split("\n").collect::<Vec<&str>>()[0].to_string();

        // get faq content
        let faq_content = &content.split("\n").collect::<Vec<&str>>()[1..].join("\n");

        //log!("faq_content: {:?}", faq_content);
        //log!("content: {:?}", content);

        faqs.push(FAQ::new_faq(id, title.to_string(), faq_content.to_string()));
    }

    Ok(faqs)
}
