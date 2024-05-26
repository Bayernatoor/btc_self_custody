#[cfg(feature = "ssr")]
use {
    actix_web::{web, HttpResponse},
    chrono::Utc,
    sqlx::PgPool,
    tracing_futures::Instrument,
    uuid::Uuid,
};

#[cfg(feature = "ssr")]
#[derive(serde::Deserialize)]
pub struct BlogpostData {
    author: String,
    title: String,
    content: String,
}

// creates a span at the beginning of the function invocation
#[tracing::instrument(
    name ="Adding new blogpost to database",
    skip(blogpost_data, pool),
    fields(
        request_id = %Uuid::new_v4(),
        %blogpost_data.title
        )
    )]
#[cfg(feature = "ssr")]
pub async fn create_post(
    blogpost_data: web::Json<BlogpostData>,
    pool: web::Data<PgPool>,
) -> HttpResponse {
    match insert_post(&pool, &blogpost_data).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

#[tracing::instrument(
    name = "Saving new blogpost details to database"
    skip(blogpost_data, pool)
    )]
#[cfg(feature = "ssr")]
pub async fn insert_post(
    pool: &PgPool,
    blogpost_data: &BlogpostData,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        insert into blogposts (id, created_at, author, title, content)
        values ($1, $2, $3, $4, $5)
        "#,
        Uuid::new_v4(),
        Utc::now(),
        blogpost_data.author,
        blogpost_data.title,
        blogpost_data.content
    )
    // use get_ref to get immutable reference to the pgconnection
    // wrapped by web::data
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;
    Ok(())
}
