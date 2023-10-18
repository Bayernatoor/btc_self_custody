use std::fs;
use leptos::*;
use leptos::logging::log;
use serde::{Serialize, Deserialize};


#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FAQ {
    pub id: u32, 
    pub title: String,
    pub content: String,
}

impl FAQ {
    pub fn new_faq(id: u32, title: String, content: String) -> Self {
        Self {
            id,
            title,
            content,
        }
    }
}
#[server(FetchFaq, "/api")]
pub async fn fetch_faq(wallet: String) -> Result<Vec<FAQ>, ServerFnError> {

    let path = format!("./src/faqs/{}", wallet);

    let faqs_dir = fs::read_dir(path)?;

    let mut faqs = Vec::new();
    let mut id = 0;

    for faq in faqs_dir {
        let faq = faq?;
        //log!("files in dir:  {:?}", faq);
        let path = faq.path();
        //log!("path buffer:  {:?}", path);
        // increment id for each new file
        id += 1;
        // get name of file
        let file_name = path.file_name().unwrap().to_str().unwrap();
        // read contents of file 
        let content = fs::read_to_string(&path)?;  
        // get the faq title
        let title = content.split("\n").collect::<Vec<&str>>()[0].to_string();


        faqs.push(FAQ::new_faq(id, title, file_name.to_string()));

    }; 
    
    log!("FetchFaq Returns: {:?}", faqs);
    Ok(faqs)

    //go into directory and read each file
    //for each file I need to take the first line and assign it to the title
    //the remainder of the lines are content
    //create a new FAQ and push it to a faqs vec.
    //function returns a result containing a Vec<FAQ>
   



}


 
