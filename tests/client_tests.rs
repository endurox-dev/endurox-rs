use endurox_rs::Client;

#[test]
fn test_client_init_integration() {
    let client = Client::init();
    assert!(client.is_ok());
}

