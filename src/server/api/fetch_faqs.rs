use std::fs;
use leptos::*;
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
pub async fn fetch_faq() -> Result<Vec<FAQ>, ServerFnError> {

    let path = "../../faqs/samourai/";

    let faqs_dir = fs::read_dir(path)?;

    let mut faqs = Vec::new();

    for faq in faqs_dir {
        let faq = faq?;
        let path = faq.path();
        let file_name = path.file_name().unwrap().to_str().unwrap();
        
        faqs.push(FAQ::new_faq(1, file_name.to_string(), file_name.to_string()));

    }; 
    
    Ok(faqs)

    //go into directory and read each file
    //for each file I need to take the first line and assign it to the title
    //the remainder of the lines are content
    //    
    //each file will be a FAQ struct 

    //iterrate over the files, take contents and build a new faq push faq into 
    //a vec 

    //vec is then returned 


}


 
