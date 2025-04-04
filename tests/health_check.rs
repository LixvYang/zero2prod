// //! tests/health_check.rs
// use once_cell::sync::Lazy;
// use sqlx::{Connection, Executor, PgConnection, PgPool};
// use std::net::TcpListener;
// use uuid::Uuid;
// use zero2prod::configuration::{get_configuration, DatabaseSettings};
// use zero2prod::email_client::EmailClient;
// use zero2prod::startup::run;
// use zero2prod::telemetry::{get_subscriber, init_subscriber};

// // Ensure that the `tracing` stack is only initialised once using `once_cell`
// static TRACING: Lazy<()> = Lazy::new(|| {
//     let default_filter_level = "info".to_string();
//     let subscriber_name = "test".to_string();
//     // We cannot assign the output of `get_subscriber` to a variable based on the value of `TEST_LOG`
//     // because the sink is part of the type returned by `get_subscriber`, therefore they are not the
//     // same type. We could work around it, but this is the most straight-forward way of moving forward.
//     if std::env::var("TEST_LOG").is_ok() {
//         let subscriber = get_subscriber(&subscriber_name, &default_filter_level, std::io::stdout);
//         init_subscriber(subscriber);
//     } else {
//         let subscriber = get_subscriber(&subscriber_name, &default_filter_level, std::io::sink);
//         init_subscriber(subscriber);
//     };
// });

// pub struct TestApp {
//     pub address: String,
//     pub db_pool: PgPool,
// }

// // The function is asynchronous now!
// async fn spawn_app() -> TestApp {
//     Lazy::force(&TRACING);

//     let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
//     let port = listener.local_addr().unwrap().port();
//     let address = format!("http://127.0.0.1:{}", port);

//     let mut configuration = get_configuration().expect("Failed to read configuration.");
//     configuration.database.database_name = Uuid::new_v4().to_string();

//     // Build a new email client
//     let sender_email = configuration
//         .email_client
//         .sender()
//         .expect("Invalid sender email address.");
//     let timeout = configuration.email_client.timeout();
//     let email_client = EmailClient::new(
//         configuration.email_client.base_url,
//         sender_email,
//         configuration.email_client.authorization_token,
//         timeout,
//     );

//     let connection_pool = configure_database(&configuration.database).await;
//     let server =
//         run(listener, connection_pool.clone(), email_client).expect("Failed to bind address");
//     let _ = tokio::spawn(server);
//     TestApp {
//         address,
//         db_pool: connection_pool,
//     }
// }

// #[actix_rt::test]
// async fn health_check_works() {
//     // Arrange
//     let app = spawn_app().await;
//     let client = reqwest::Client::new();

//     // Act
//     let response = client
//         // Use the returned application address
//         .get(&format!("{}/health_check", &app.address))
//         .send()
//         .await
//         .expect("Failed to execute request.");

//     // Assert
//     assert!(response.status().is_success());
//     assert_eq!(Some(0), response.content_length());
// }

// // // The function is asynchronous now!
// // async fn spawn_app() -> TestApp {
// //     let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
// //     let port = listener.local_addr().unwrap().port();

// //     let address = format!("http://127.0.0.1:{}", port);

// //     let configuration = get_configuration().expect("Failed to read configuration.");
// //     let connection_pool = PgPool::connect(&configuration.database.connection_string())
// //         .await
// //         .expect("Failed to connect to Postgres.");

// //     let server = run(listener, connection_pool.clone()).expect("Failed to bind address");
// //     let _ = tokio::spawn(server);
// //     TestApp {
// //         address,
// //         db_pool: connection_pool,
// //     }
// // }

// // #[test]
// // fn dummy_fail() {
// //     let result: Result<&str, &str> = Err("The app crashed due to an IO error");
// //     claim::assert_ok!(result);
// // }
