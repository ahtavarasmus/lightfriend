//! One-shot helper to force Tesla Fleet API to re-fetch our partner public key.
//!
//! Background: when our public key rotates (e.g. an unpaired deploy regenerated
//! it before the tesla_keys.rs derive-from-private fix), Tesla still serves the
//! cached old public key for partner lookups, so virtual-key pairing fails with
//! "not registered". The Tesla Fleet API has a dedicated endpoint for forcing a
//! refresh: `POST /api/1/partner_accounts/public_key?domain=<domain>`. Our
//! backend only ever hits `POST /api/1/partner_accounts` (the registration
//! endpoint), which does NOT refresh the cached key. This binary fills that
//! gap as a one-off.
//!
//! Usage:
//!   cd backend && cargo run --bin refresh_tesla_partner
//!
//! Uses backend/.env for TESLA_CLIENT_ID / TESLA_CLIENT_SECRET /
//! TESLA_REDIRECT_URL. Calls the public_key refresh endpoint in all three
//! Tesla regions (EU, NA, AP) so it doesn't matter which one our partner is
//! homed in. Safe to re-run.

use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();

    let client_id = std::env::var("TESLA_CLIENT_ID")
        .map_err(|_| "TESLA_CLIENT_ID not set in environment".to_string())?;
    let client_secret = std::env::var("TESLA_CLIENT_SECRET")
        .map_err(|_| "TESLA_CLIENT_SECRET not set in environment".to_string())?;
    let redirect_url = std::env::var("TESLA_REDIRECT_URL")
        .or_else(|_| std::env::var("SERVER_URL"))
        .or_else(|_| std::env::var("SERVER_URL_OAUTH"))
        .map_err(|_| "TESLA_REDIRECT_URL not set in environment".to_string())?;

    let domain = redirect_url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .split('/')
        .next()
        .unwrap_or("");

    if domain.is_empty() {
        return Err("Could not derive domain from TESLA_REDIRECT_URL".into());
    }

    println!("Domain: {}", domain);

    let http = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let regions = [
        ("EU", "https://fleet-api.prd.eu.vn.cloud.tesla.com"),
        ("NA", "https://fleet-api.prd.na.vn.cloud.tesla.com"),
        ("AP", "https://fleet-api.prd.ap.vn.cloud.tesla.com"),
    ];

    for (name, base_url) in regions {
        println!("\n=== Region: {} ({}) ===", name, base_url);

        // 1. Mint a partner (client_credentials) token scoped to this region's
        //    audience. Region matters: token's audience binds it to one region.
        let token_url = "https://fleet-auth.prd.vn.cloud.tesla.com/oauth2/v3/token";
        let token_params = [
            ("grant_type", "client_credentials"),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            (
                "scope",
                "openid vehicle_device_data vehicle_cmds vehicle_charging_cmds",
            ),
            ("audience", base_url),
        ];
        let token_resp = http.post(token_url).form(&token_params).send().await?;
        if !token_resp.status().is_success() {
            let body = token_resp.text().await.unwrap_or_default();
            eprintln!("  ✗ token request failed: {}", body);
            continue;
        }
        let token_json: serde_json::Value = token_resp.json().await?;
        let access_token = match token_json["access_token"].as_str() {
            Some(t) => t.to_string(),
            None => {
                eprintln!("  ✗ no access_token in response");
                continue;
            }
        };
        println!("  ✓ partner token minted");

        // 2. Re-fetch step. Two ways Tesla exposes this:
        //    a) POST /api/1/partner_accounts/public_key (with domain query) —
        //       documented but not universally available across regions; if a
        //       region 404s, we fall through.
        //    b) POST /api/1/partner_accounts (re-registration) — always
        //       available and observed in practice to trigger a re-fetch from
        //       /.well-known/.../com.tesla.3p.public-key.pem on Tesla's side.
        //
        // We do both. Either succeeding is enough.
        let pk_url = format!(
            "{}/api/1/partner_accounts/public_key?domain={}",
            base_url, domain
        );
        let pk_resp = http
            .post(&pk_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?;
        let pk_status = pk_resp.status();
        let pk_body = pk_resp.text().await.unwrap_or_default();
        println!("  public_key refresh: HTTP {}", pk_status);
        if !pk_body.is_empty() {
            println!("    body: {}", truncate(&pk_body, 400));
        }

        let reg_url = format!("{}/api/1/partner_accounts", base_url);
        let reg_body = serde_json::json!({"domain": domain});
        let reg_resp = http
            .post(&reg_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Type", "application/json")
            .json(&reg_body)
            .send()
            .await?;
        let reg_status = reg_resp.status();
        let reg_body = reg_resp.text().await.unwrap_or_default();
        println!("  partner_accounts re-register: HTTP {}", reg_status);
        if !reg_body.is_empty() {
            // Print full body so we can see the full public_key field.
            println!("    body: {}", reg_body);
        }

        // 3. GET what Tesla currently has cached for this partner.
        let get_url = format!(
            "{}/api/1/partner_accounts/public_key?domain={}",
            base_url, domain
        );
        let get_resp = http
            .get(&get_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?;
        let get_status = get_resp.status();
        let get_body = get_resp.text().await.unwrap_or_default();
        println!("  GET cached public_key: HTTP {}", get_status);
        if !get_body.is_empty() && get_body.len() < 2000 {
            println!("    body: {}", get_body);
        }
    }

    println!("\nDone. Try pairing the virtual key again now.");
    Ok(())
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n])
    }
}
