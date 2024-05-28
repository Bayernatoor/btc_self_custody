#[cfg(feature = "ssr")]
use {
    actix_web::{web, HttpResponse},
    chrono::Utc,
    sqlx::PgPool,
    uuid::Uuid,
    tracing_futures::Instrument,
};

#[cfg(feature = "ssr")]
#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

// creates a span at the beginning of the function invocation
#[tracing::instrument(
    name ="Adding a new subscriber",
    skip(form, pool),
    fields(
        request_id = %Uuid::new_v4(),
        subscriber_email = %form.email,
        subscriber_name = %form.name,
        )
    )]
#[cfg(feature = "ssr")]
pub async fn subscribe(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>, 
) -> HttpResponse {
    match insert_subscriber(&form, &pool).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

#[tracing::instrument(
    name = "Saving new blogpost details to database"
    skip(form, pool)
    )]
#[cfg(feature = "ssr")]
pub async fn insert_subscriber(
    form: &FormData,
    pool: &PgPool,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at)
        values ($1, $2, $3, $4)
        "#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now(),
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
