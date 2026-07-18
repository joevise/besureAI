use besure_lib::crypto::VaultCrypto;

#[test]
fn test_real_vault_auth() {
    let home = std::env::var("HOME").unwrap();
    let config_str = std::fs::read_to_string(format!("{}/.besure/.besure.config", home)).unwrap();
    let config: serde_json::Value = serde_json::from_str(&config_str).unwrap();
    let salt: Vec<u8> = serde_json::from_value(config["salt"].clone()).unwrap();
    let verify: Vec<u8> = serde_json::from_value(config["verify_token"].clone()).unwrap();
    
    let mut crypto = VaultCrypto::from_salt(salt);
    let result = crypto.unlock_with_verify("besure2026", &verify).unwrap();
    assert!(result, "password besure2026 should verify against real vault");
}
