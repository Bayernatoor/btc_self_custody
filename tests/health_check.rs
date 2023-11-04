#[tokio::test]
async fn health_check_works() {
    
    // create an app
    spawn_app().await.expect("Failed to spawn our app");

    // Use reqwest to perform HTTP actions against our app
    let client = reqwest::Client::new();
    
    // Act
    let response = client 
            .get("http:://127.0.0.1:3000/health_check")
            .send()
            .await
            .expect("Failed to execute request");

    // Assert
    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}


async fn spawn_app() -> std::io::Result<()> {
    todo!()    

}
