#[tokio::test]
#[cfg(feature = "ssr")]
async fn health_check_works() {
    // create an app
    spawn_app();

    // Use reqwest to perform HTTP actions against our app
    let client = reqwest::Client::new();

    // Act
    let response = client
        .get("http://127.0.0.1:3000/server/health_check")
        .send()
        .await
        .expect("Failed to execute request");

    // Assert
    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

#[cfg(feature = "ssr")]
async fn spawn_app() -> std::io::Result<()> {
    let server = btc_self_custody::run()
        .await
        .expect("Failed to bind to address");
    let _ = tokio::spawn(server);
    Ok(())
}
