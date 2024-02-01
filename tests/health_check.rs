#[cfg(feature = "ssr")]
use btc_self_custody::configuration::{get_configuration, DatabaseSettings};
#[cfg(feature = "ssr")]
use sqlx::{Connection, Executor, PgConnection, PgPool};
#[cfg(feature = "ssr")]
use uuid::Uuid;
#[cfg(feature = "ssr")]
pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
}

//pub struct BlogPost {
//    pub id: String,
//    created_at: .
//    created_at timestamptz NOT NULL,
//    update_at: ,
//    updated_at timestamptz,
//    title: String,
//    title VARCHAR(255) NOT NULL,
//    subtitle: String,
//    subtitle VARCHAR(255),
//    author: String,
//    author TEXT NOT NULL,
//    content: String,
//    content TEXT NOT NULL,
//    excerpt: String,
//    excerpt TEXT,
//    tags: Vec<String>,
//    tags TEXT[],
//    status: ,
//    status blogpost_status DEFAULT 'draft',
//    slug: String,
//    slug VARCHAR(255) UNIQUE,
//    views: u32,
//    views INT DEFAULT 0,
//    comments_count: u32,
//    comments_count INT DEFAULT 0
//}

#[tokio::test]
#[cfg(feature = "ssr")]
async fn create_returns_a_200_for_valid_post_creation() {
    use sqlx::{Connection, PgConnection};
    use std::collections::HashMap;

    // Arrange
    let mut configuration =
        get_configuration().expect("Failed to read configuration.");
    configuration.database.database_name = Uuid::new_v4().to_string();

    let connection_pool = configure_database(&configuration.database).await;

    let client = reqwest::Client::new();

    let mut map = HashMap::new();
    map.insert("author", "Bayer");
    map.insert("title", "The path to Hyperbitcoinization");
    map.insert("content", "We explore the many...");

    // Act
    let response = client
        .post("http://127.0.0.1:3000/server/create_post")
        .header("Content-Type", "application/json")
        .json(&map)
        .send()
        .await
        .expect("Failed to execute request.");

    // Assert
    assert_eq!(200, response.status().as_u16());

    let saved = sqlx::query!("SELECT title FROM blogposts")
        .fetch_one(&connection_pool)
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
    //let app_address = spawn_app();
    //let configuration = get_configuration().expect("Failed to read configuration");
    //configuration.database.database_name = Uuid::new_v4().to_string();

    //let connection = PgConnection::connect(&configuration.database.connection_string())
    //    .await
    //    .expect("Failed to connect to Postgres.");

    let mut configuration =
        get_configuration().expect("Failed to read configuration.");
    configuration.database.database_name = Uuid::new_v4().to_string();

    let connection_pool = configure_database(&configuration.database).await;

    let client = reqwest::Client::new();

    let mut map = HashMap::new();

    map.insert("author", "");
    map.insert("content", "Hyperbitcoinization, the point at which Bitcoin becomes the dominant world reserve currency, was originally coined by Daniel Krawisz in his 2014 article titled Hyperbitcoinization.");

    // Act
    let response = client
        .post("http://127.0.0.1:3000/server/create_post")
        .header("Content-Type", "application/json")
        .json(&map)
        .send()
        .await
        .expect("Failed to execute request.");

    // Assert
    assert_eq!(400, response.status().as_u16());
}

//#[tokio::test]
//#[cfg(feature = "ssr")]
//async fn fetch_returns_a_200_for_valid_post_retrieval() {
//    use std::collections::HashMap;
//
//    // Arrange
//    let app_address = spawn_app();
//    let configuration = get_configuration().expect("Failed to read configuration");
//    let connection = PgConnection::connect(&configuration.database.connection_string())
//        .await
//        .expect("Failed to connect to Postgres.");
//
//    let client = reqwest::Client::new();
//
//    // Act
//    let response = client
//        .get("http://127.0.0.1:3000/server/retrieve_post")
//        .header("Content-Type", "application/json")
//        .send()
//        .await
//        .expect("Failed to execute request.");
//
//    println!("response: {:?}", response)
//
//    // Assert
//    assert_eq!(200, response.status().as_u16());
//
//}

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
    use btc_self_custody::configuration;

    let address = format!("http://127.0.0.1:3000");

    let mut configuration =
        get_configuration().expect("Failed to read configuration.");
    configuration.database.database_name = Uuid::new_v4().to_string();

    let connection_pool = configure_database(&configuration.database).await;

    let server = btc_self_custody::run(connection_pool.clone())
        .await
        .expect("Failed to bind to address");
    let _ = tokio::spawn(server);

    TestApp {
        address,
        db_pool: connection_pool,
    };

    Ok(())
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
