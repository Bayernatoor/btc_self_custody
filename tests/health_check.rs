#[actix_rt::test]
async fn health_check_works() {
    
    // create an app
    spawn_app().await;


    // Use reqwest to perform HTTP actions against our app
    let client = reqwest::Client::new();

    // Act
    let response = client
        .get("http://127.0.0.1:3000/health_check")
        .send()
        .await
        .expect("Failed to execute request");

    // Assert
    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}


async fn spawn_app() {
    let server = btc_self_custody::run("127.0.0.1:0").await.expect("Failed to bind address");
    let _ = tokio::spawn(server);
}
