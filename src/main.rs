//! src/main.rs
use sqlx::postgres::PgPoolOptions;
use std::net::TcpListener;
use zero2prod::configuration::get_configuration;
use zero2prod::email_client::EmailClient;
use zero2prod::startup::{build, run, Application};
use zero2prod::telemetry;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let subscriber = telemetry::get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    telemetry::init_subscriber(subscriber);

    let configuration = get_configuration().expect("Failed to read configuration.");
    // Renamed!
    let connection_pool = PgPoolOptions::new().connect_lazy_with(configuration.database.with_db());

    // let address = format!(
    //     "{}:{}",
    //     configuration.application.host, configuration.application.port
    // );
    // let timeout = configuration.email_client.timeout();
    // // Build an `EmailClient` using `configuration`
    // let sender_email = configuration
    //     .email_client
    //     .sender()
    //     .expect("Invalid sender email address.");
    // let email_client = EmailClient::new(
    //     configuration.email_client.base_url,
    //     sender_email,
    //     configuration.email_client.authorization_token,
    //     timeout,
    // );

    // let listener = TcpListener::bind(address)?;
    // run(listener, connection_pool, email_client)?.await
    let configuration = get_configuration().expect("Failed to read configuration.");
    let application = Application::build(configuration).await?;
    application.run_until_stopped().await?;
    Ok(())
}
