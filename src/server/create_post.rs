#[cfg(feature = "ssr")]
use actix_web::{web, HttpResponse};
#[cfg(feature = "ssr")]
use sqlx::PgPool;

#[cfg(feature = "ssr")]
#[derive(serde::Deserialize)]
pub struct BlogpostData {
    author: String,
    title: String,
    content: String,
}

#[cfg(feature = "ssr")]
pub async fn create_post(
    blogpost_data: web::Json<BlogpostData>,
    connection: web::Data<PgPool>,
) -> HttpResponse {
    use chrono::Utc;
    use uuid::Uuid;

    match sqlx::query!(
        r#"
        INSERT INTO blogposts (id, created_at, author, title, content)
        VALUES ($1, $2, $3, $4, $5)
        "#,
        Uuid::new_v4(),
        Utc::now(),
        blogpost_data.author,
        blogpost_data.title,
        blogpost_data.content
    )
    // use get_ref to get immutable reference to the PgConnection
    // wrapped by web::Data
    .execute(connection.get_ref())
    .await
    {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => {
            println!("Failed to execute query: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}
