use crate::{
    domain::{NewSubscriber, SubscriberEmail, SubscriberName},
    email_client::EmailClient,
    startup::ApplicationBaseUrl,
};
use actix_web::{web, HttpResponse};
use chrono::Utc;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use sqlx::{Executor, PgPool, Postgres, Transaction};
use std::convert::{TryFrom, TryInto};
use uuid::Uuid;

impl TryFrom<FormData> for NewSubscriber {
    type Error = String;

    fn try_from(value: FormData) -> Result<Self, Self::Error> {
        let name = SubscriberName::parse(value.name)?;
        let email = SubscriberEmail::parse(value.email)?;
        Ok(Self { email, name })
    }
}

// We derive `Debug`, easy and painless.
// pub struct StoreTokenError(sqlx::Error);

// impl std::fmt::Display for StoreTokenError {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(
//             f,
//             "A database error was encountered while \
//             trying to store a subscription token."
//         )
//     }
// }

// impl std::fmt::Debug for StoreTokenError {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         error_chain_fmt(self, f)
//     }
// }

// #[derive(Debug)]
// pub enum SubscribeError {
//     ValidationError(String),
//     DatabaseError(sqlx::Error),
//     StoreTokenError(StoreTokenError),
//     SendEmailError(reqwest::Error),
//     PoolError(#[source] sqlx::Error),
//     InsertSubscriberError(#[source] sqlx::Error),
//     TransactionCommitError(#[source] sqlx::Error),
// }

// #[derive(thiserror::Error)]
// pub enum SubscribeError {
//     #[error("{0}")]
//     ValidationError(String),
//     #[error("Failed to acquire a Postgres connection from the pool")]
//     PoolError(#[source] sqlx::Error),
//     #[error("Failed to insert new subscriber in the database.")]
//     InsertSubscriberError(#[source] sqlx::Error),
//     #[error("Failed to store the confirmation token for a new subscriber.")]
//     StoreTokenError(#[from] StoreTokenError),
//     #[error("Failed to commit SQL transaction to store a new subscriber.")]
//     TransactionCommitError(#[source] sqlx::Error),
//     #[error("Failed to send a confirmation email.")]
//     SendEmailError(#[from] reqwest::Error),
// }

// impl std::fmt::Display for SubscribeError {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "Failed to create a new subscriber.")
//     }
// }

// impl std::error::Error for SubscribeError {
//     fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
//         match self {
//             Self::ValidationError(_) => None,
//             Self::DatabaseError(e) => Some(e),
//             Self::StoreTokenError(_) => None, // StoreTokenError doesn't implement std::error::Error
//             Self::SendEmailError(e) => Some(e),
//         }
//     }
// }

// impl ResponseError for SubscribeError {}

// impl From<reqwest::Error> for SubscribeError {
//     fn from(e: reqwest::Error) -> Self {
//         Self::SendEmailError(e)
//     }
// }

// impl From<sqlx::Error> for SubscribeError {
//     fn from(e: sqlx::Error) -> Self {
//         Self::DatabaseError(e)
//     }
// }

// impl From<StoreTokenError> for SubscribeError {
//     fn from(e: StoreTokenError) -> Self {
//         Self::StoreTokenError(e)
//     }
// }

// impl From<String> for SubscribeError {
//     fn from(e: String) -> Self {
//         Self::ValidationError(e)
//     }
// }

fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, pool, email_client, base_url),
    fields(
        subscriber_email = %form.email,
        subscriber_name= %form.name
    )
)]
pub async fn subscribe(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    base_url: web::Data<ApplicationBaseUrl>,
) -> HttpResponse {
    let new_subscriber = match form.0.try_into() {
        Ok(form) => form,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    let mut transaction = match pool.begin().await {
        Ok(transaction) => transaction,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    // let mut transaction = pool.begin().await.map_err(|e| {
    //     tracing::error!(
    //         "Failed to acquire a Postgres connection from the pool: {:?}",
    //         e
    //     );
    //     HttpResponse::InternalServerError().finish()
    // });

    let subscriber_id = match insert_subscriber(&mut transaction, &new_subscriber).await {
        Ok(subscriber_id) => subscriber_id,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };
    let subscription_token = generate_subscription_token();
    if store_token(&mut transaction, subscriber_id, &subscription_token)
        .await
        .is_err()
    {
        return HttpResponse::InternalServerError().finish();
    }
    if transaction.commit().await.is_err() {
        return HttpResponse::InternalServerError().finish();
    }

    if send_confirmation_email(
        &email_client,
        new_subscriber,
        &base_url.0,
        &subscription_token,
    )
    .await
    .is_err()
    {
        return HttpResponse::InternalServerError().finish();
    }
    HttpResponse::Ok().finish()
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(new_subscriber, transaction)
)]
pub async fn insert_subscriber(
    transaction: &mut Transaction<'_, Postgres>,
    new_subscriber: &NewSubscriber,
) -> Result<Uuid, sqlx::Error> {
    let subscriber_id = Uuid::new_v4();
    let query = sqlx::query!(
        r#"
    INSERT INTO subscriptions (id, email, name, subscribed_at, status)
    VALUES ($1, $2, $3, $4, 'pending_confirmation')
            "#,
        subscriber_id,
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(),
        Utc::now()
    );

    transaction.execute(query).await?;
    Ok(subscriber_id)
}

#[tracing::instrument(
    name = "Send a confirmation email to a new subscriber",
    skip(email_client, new_subscriber, base_url, subscription_token)
)]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: NewSubscriber,
    base_url: &str,
    // New parameter!
    subscription_token: &str,
) -> Result<(), reqwest::Error> {
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        base_url, subscription_token
    );
    let plain_body = format!(
        "Welcome to our newsletter!\nVisit {} to confirm your subscription.",
        confirmation_link
    );
    let html_body = format!(
        "Welcome to our newsletter!<br />\
        Click <a href=\"{}\">here</a> to confirm your subscription.",
        confirmation_link
    );
    email_client
        .send_email(new_subscriber.email, "Welcome!", &html_body, &plain_body)
        .await
}

/// Generate a random 25-characters-long case-sensitive subscription token.
fn generate_subscription_token() -> String {
    let mut rng = thread_rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}

#[tracing::instrument(
    name = "Store subscription token in the database",
    skip(subscription_token, transaction)
)]
pub async fn store_token(
    transaction: &mut Transaction<'_, Postgres>,
    subscriber_id: Uuid,
    subscription_token: &str,
) -> Result<(), String> {
    let query = sqlx::query!(
        r#"
    INSERT INTO subscription_tokens (subscription_token, subscriber_id)
    VALUES ($1, $2)
        "#,
        subscription_token,
        subscriber_id
    );
    transaction
        .execute(query)
        .await
        .map_err(|err| format!("Failed to store token in the database: {:?}", err))?;
    Ok(())
}
