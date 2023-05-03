//! Test things
//!

use std::env;

use anyhow::Context;
use log::info;

#[tokio::test]
async fn test_unread_count() {
    if let Err(_) = flexi_logger::Logger::try_with_str("TRACE").unwrap().start() {};
    let username =
        env::var("GOOGLE_READER_USERNAME").expect("Missing env var: GOOGLE_READER_USERNAME");
    let password =
        env::var("GOOGLE_READER_PASSWORD").expect("Missing env var: GOOGLE_READER_PASSWORD");
    let server = env::var("GOOGLE_READER_SERVER").expect("Missing env var: GOOGLE_READER_SERVER");

    let mut reader = super::GoogleReader::try_new(username, password, &server)
        .expect("Failed to create API object");

    let res = reader.unread_count().await;
    info!("{:?}", res);
    if server.contains("api/greader.php") {
        assert!(res.is_err());
    } else {
        assert!(res.is_ok());
    }
}

#[tokio::test]
async fn test_get_write_token() {
    if let Err(_) = flexi_logger::Logger::try_with_str("TRACE").unwrap().start() {};
    let username =
        env::var("GOOGLE_READER_USERNAME").expect("Missing env var: GOOGLE_READER_USERNAME");
    let password =
        env::var("GOOGLE_READER_PASSWORD").expect("Missing env var: GOOGLE_READER_PASSWORD");
    let server = env::var("GOOGLE_READER_SERVER").expect("Missing env var: GOOGLE_READER_SERVER");

    let mut reader = super::GoogleReader::try_new(username, password, server)
        .expect("Failed to create API object");

    let write_token = reader
        .get_write_token()
        .await
        .with_context(|| "Failed to get write_token")
        .unwrap();

    info!("Write token: {:?}", write_token);

    // let unread_ids = reader.get_unread_items().await.with_context(|| "Failed to query unread ids").unwrap();

    // for item in unread_ids {
    //     let unread = reader.get_item(item).await;
    //     info!("Unread ID: {:?}", unread);
    // }
}
#[tokio::test]
async fn test_get_unread_items() {
    if let Err(_) = flexi_logger::Logger::try_with_str("TRACE").unwrap().start() {};
    let username =
        env::var("GOOGLE_READER_USERNAME").expect("Missing env var: GOOGLE_READER_USERNAME");
    let password =
        env::var("GOOGLE_READER_PASSWORD").expect("Missing env var: GOOGLE_READER_PASSWORD");
    let server = env::var("GOOGLE_READER_SERVER").expect("Missing env var: GOOGLE_READER_SERVER");

    let mut reader = super::GoogleReader::try_new(username, password, server)
        .expect("Failed to create API object");

    let unread_response = reader
        .get_unread_items(None)
        .await
        .with_context(|| "Failed to query unread ids")
        .unwrap();

    unread_response.items.iter().for_each(|item| {
        info!("Unread: {:?}", item);
    });

    info!("Got {} items", unread_response.items.len());

    match unread_response.continuation {
        Some(_) => info!("Got continuation response, need to query again!"),
        None => info!("No continuation response, we're done!"),
    }
}
