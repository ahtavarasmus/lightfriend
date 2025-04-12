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
        let nonce_res = self
            .http_client
            .get(format!("{}/_synapse/admin/v1/register", self.homeserver))
            .send()
            .await
            .map_err(|e| anyhow!("Failed to fetch nonce: {}", e))?
            .json::<serde_json::Value>()
            .await
            .map_err(|e| anyhow!("Failed to parse nonce response: {}", e))?;
        let nonce = nonce_res["nonce"]
            .as_str()
            .ok_or_else(|| anyhow!("No nonce in response"))?;
        println!("📝 Got registration nonce: {}", nonce);

        // Generate unique username and password
        let username = format!("appuser_{}", Uuid::new_v4().to_string().replace("-", ""));
        let password = Uuid::new_v4().to_string();
        println!("👤 Generated username: {}", username);
        println!("🔑 Generated password: [hidden]");

        // Calculate MAC
        let mac_content = format!("{}\0{}\0{}\0notadmin", nonce, username, password);
        println!("🔒 MAC content: {}", mac_content);
        let mut mac = Hmac::<Sha1>::new_from_slice(self.shared_secret.as_bytes())
            .map_err(|e| anyhow!("Failed to create HMAC: {}", e))?;
        mac.update(mac_content.as_bytes());
        let mac_result = hex::encode(mac.finalize().into_bytes());
        println!("🔐 Generated MAC: {}", mac_result);

        // Register user
        println!("📡 Sending registration request to Matrix server...");
        let response = self
            .http_client
            .post(format!("{}/_synapse/admin/v1/register", self.homeserver))
            .json(&json!({
                "nonce": nonce,
                "username": username,
                "password": password,
                "admin": false,
                "mac": mac_result
            }))
            .send()
            .await
            .map_err(|e| anyhow!("Failed to send registration request: {}", e))?;

        // Log status
        let status = response.status();
        println!("📡 Registration response status: {}", status);

        // Get response body
        let register_res = response
            .text()
            .await
            .map_err(|e| anyhow!("Failed to read response body: {}", e))?;
        println!("📡 Registration response body: {}", register_res);

        let register_json: serde_json::Value = serde_json::from_str(&register_res)
            .map_err(|e| anyhow!("Failed to parse registration response: {}", e))?;

        if status.is_success() {
            let access_token = register_json["access_token"]
                .as_str()
                .ok_or_else(|| anyhow!("No access_token in response: {}", register_res))?
                .to_string();
            let device_id = register_json["device_id"]
                .as_str()
                .ok_or_else(|| anyhow!("No device_id in response: {}", register_res))?
                .to_string();
            println!("✅ Matrix registration successful!");
            println!("📱 Device ID: {}", device_id);
            println!("🎫 Access token received (length: {})", access_token.len());
            Ok((username, access_token, device_id))
        } else {
            let error = register_json["error"]
                .as_str()
                .unwrap_or("Unknown error");
            Err(anyhow!("Registration failed: {} (status: {})", error, status))
        }
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

