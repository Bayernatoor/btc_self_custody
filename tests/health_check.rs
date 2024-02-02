#[cfg(feature = "ssr")]
use btc_self_custody::configuration::{get_configuration, DatabaseSettings};
#[cfg(feature = "ssr")]
use btc_self_custody::run;
#[cfg(feature = "ssr")]
use sqlx::{Connection, Executor, PgConnection, PgPool};
#[cfg(feature = "ssr")]
use std::net::TcpListener;
#[cfg(feature = "ssr")]
use uuid::Uuid;

#[cfg(feature = "ssr")]
pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
}

#[cfg(feature = "ssr")]
async fn spawn_app() -> TestApp {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
    // retrieve random port assigned to us by OS
    let port = listener.local_addr().unwrap().port();
    println!("spawn app port: {:?}", &port);
    let address = format!("http://127.0.0.1:{}", port);

    let mut configuration =
        get_configuration().expect("Failed to read configuration.");
    configuration.database.database_name = Uuid::new_v4().to_string();
    let connection_pool = configure_database(&configuration.database).await;

    let server = run(listener, connection_pool.clone()).await.expect("Failed to bind to address");
    let _ = tokio::spawn(server);

    TestApp {
        address,
        db_pool: connection_pool,
    }


}

#[cfg(feature = "ssr")]
pub async fn configure_database(config: &DatabaseSettings) -> PgPool {
    // create database
    let mut connection =
        PgConnection::connect(&config.connection_string_without_db())
            .await
            .expect("Failed to connect to Postgres");
    connection
        .execute(
            format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str(),
        )
        .await
        .expect("Failed to create database.");

    // Migrate Database
    let connection_pool = PgPool::connect(&config.connection_string())
        .await
        .expect("Faied to connect to Postgres");
    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migration the database");

    connection_pool
}

#[tokio::test]
#[cfg(feature = "ssr")]
async fn health_check_works() {
    
    // Arrange
    let app = spawn_app().await;
    // Use reqwest to perform HTTP actions against our app
    let client = reqwest::Client::new();
    

    // Act
    let response = client
        .get(&format!("{}/server/health_check", &app.address))
        .send()
        .await
        .expect("Failed to execute request");

    // Assert
    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

#[tokio::test]
#[cfg(feature = "ssr")]
async fn create_returns_a_200_for_valid_post_creation() {
    use sqlx::{Connection, PgConnection};
    use std::collections::HashMap;

    // Arrange
    let app = spawn_app().await;
    let client = reqwest::Client::new();
    let mut map = HashMap::new();
    map.insert("author", "Bayer");
    map.insert("title", "The path to Hyperbitcoinization");
    map.insert("content", "We explore the many...");

    // Act
    let response = client
        .post(&format!("{}/server/create_post", &app.address))
        .header("Content-Type", "application/json")
        .json(&map)
        .send()
        .await
        .expect("Failed to execute request.");

    // Assert
    assert_eq!(200, response.status().as_u16());

    let saved = sqlx::query!("SELECT title FROM blogposts")
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved blog title.");

    assert_eq!(saved.title, "The path to Hyperbitcoinization");
}

#[tokio::test]
#[cfg(feature = "ssr")]
async fn create_returns_a_400_for_invalid_post_creation() {
    use std::collections::HashMap;

    use sqlx::{Connection, PgConnection};

    // Arrange

    let app = spawn_app().await;
    let client = reqwest::Client::new();

    // create hashmap with missing required values.
    let mut map = HashMap::new();
    map.insert("author", "");
    map.insert("content", "Hyperbitcoinization, the point at which Bitcoin becomes the dominant world reserve currency, was originally coined by Daniel Krawisz in his 2014 article titled Hyperbitcoinization.");

    // Act
    let response = client
        .post(&format!("{}/server/create_post", &app.address))
        .header("Content-Type", "application/json")
        .json(&map)
        .send()
        .await
        .expect("Failed to execute request.");

    // Assert
    assert_eq!(400, response.status().as_u16());
}


