use anyhow::{anyhow, Result};
use hmac::{Hmac, Mac};
use reqwest::Client as HttpClient;
use serde_json::json;
use sha1::Sha1;
use uuid::Uuid;
use magic_crypt::MagicCryptTrait;

pub struct MatrixAuth {
    homeserver: String,
    shared_secret: String,
    http_client: HttpClient,
}

impl MatrixAuth {
    pub fn new(homeserver: String, shared_secret: String) -> Self {
        Self {
            homeserver,
            shared_secret,
            http_client: HttpClient::new(),
        }
    }

    pub async fn register_user(&self) -> Result<(String, String, String)> {
        println!("🔑 Starting Matrix user registration...");
        // Get registration nonce
        let nonce_res = self.http_client.get(format!("{}/_synapse/admin/v1/register", self.homeserver))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;
        let nonce = nonce_res["nonce"].as_str().ok_or(anyhow!("No nonce"))?;
        println!("📝 Got registration nonce: {}", nonce);

        // Generate unique username and password
        let username = format!("appuser_{}", Uuid::new_v4().to_string().replace("-", ""));
        let password = Uuid::new_v4().to_string();
        println!("👤 Generated username: {}", username);

        // Calculate MAC
        let mut mac = Hmac::<Sha1>::new_from_slice(self.shared_secret.as_bytes())?;
        mac.update(format!("{}:{}:{}:{}", nonce, &username, &password, "false").as_bytes());
        let mac_result = hex::encode(mac.finalize().into_bytes());
        println!("🔐 Generated MAC for registration");

        // Register user
        println!("📡 Sending registration request to Matrix server...");
        let register_res = self.http_client.post(format!("{}/_synapse/admin/v1/register", self.homeserver))
            .json(&json!({
                "nonce": nonce,
                "username": username,
                "password": password,
                "admin": false,
                "mac": mac_result
            }))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        let access_token = register_res["access_token"].as_str().ok_or(anyhow!("No access token"))?.to_string();
        let device_id = register_res["device_id"]
            .as_str()
            .ok_or(anyhow!("No device_id in response"))?
            .to_string();
        println!("✅ Matrix registration successful!");
        println!("📱 Device ID: {}", device_id);
        println!("🎫 Access token received (length: {})", access_token.len());
        Ok((username, access_token, device_id))
    }

    pub fn encrypt_token(token: &str) -> Result<String> {
        println!("🔒 Encrypting access token...");
        let encryption_key = std::env::var("ENCRYPTION_KEY")
            .map_err(|_| anyhow!("ENCRYPTION_KEY not set"))?;
        
        let cipher = magic_crypt::new_magic_crypt!(encryption_key, 256);
        Ok(cipher.encrypt_str_to_base64(token))
    }

    pub fn decrypt_token(encrypted_token: &str) -> Result<String> {
        println!("🔓 Decrypting access token...");
        let encryption_key = std::env::var("ENCRYPTION_KEY")
            .map_err(|_| anyhow!("ENCRYPTION_KEY not set"))?;
        
        let cipher = magic_crypt::new_magic_crypt!(encryption_key, 256);
        cipher.decrypt_base64_to_string(encrypted_token)
            .map_err(|e| anyhow!("Failed to decrypt token: {}", e))
    }
}

