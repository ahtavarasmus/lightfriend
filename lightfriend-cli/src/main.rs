use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};
use keyring::Entry;
use rand::{rngs::OsRng, RngCore};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use url::Url;

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
compile_error!("lightfriend supports macOS, Linux, and Windows credential stores");

const KEYRING_SERVICE: &str = "ai.lightfriend.cli";

#[derive(Parser)]
#[command(
    name = "lightfriend",
    about = "Create Lightfriend reminders and reply watches"
)]
struct Cli {
    /// Lightfriend server. HTTPS is required except for localhost development.
    #[arg(
        long,
        env = "LIGHTFRIEND_SERVER",
        default_value = "https://lightfriend.ai"
    )]
    server: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Pair this local CLI with your Lightfriend account.
    Login {
        #[arg(long, default_value = "Local AI agent")]
        name: String,
    },
    /// Revoke this CLI credential and remove it from the OS credential store.
    Logout,
    /// Create a one-shot reminder. The time must be RFC 3339 with an offset.
    Remind {
        #[arg(long)]
        at: String,
        #[arg(long)]
        message: String,
    },
    /// Watch one email sender for the next reply (15 minutes to 24 hours).
    WatchReply {
        #[arg(long)]
        email: String,
        #[arg(long)]
        label: Option<String>,
        #[arg(long, default_value_t = 1440, value_parser = clap::value_parser!(u16).range(15..=1440))]
        for_minutes: u16,
    },
    /// Check whether a credential exists locally. No server data is read.
    Status,
}

#[derive(Serialize)]
struct StartPairingRequest<'a> {
    client_name: &'a str,
}

#[derive(Deserialize)]
struct StartPairingResponse {
    status: String,
    device_code: String,
    user_code: String,
    verification_path: String,
    expires_in: u64,
    poll_interval: u64,
}

#[derive(Serialize)]
struct PollPairingRequest<'a> {
    device_code: &'a str,
}

#[derive(Deserialize)]
struct PollPairingResponse {
    status: String,
    token: Option<String>,
}

#[derive(Serialize)]
struct ReminderRequest<'a> {
    message: &'a str,
    at: &'a str,
}

#[derive(Serialize)]
struct ReplyWatchRequest<'a> {
    email: &'a str,
    label: Option<&'a str>,
    expires_in_seconds: u32,
}

#[derive(Deserialize)]
struct ActionResponse {
    status: String,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("lightfriend: {error:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    let server = validate_server(&cli.server)?;
    let client = Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(concat!("lightfriend-cli/", env!("CARGO_PKG_VERSION")))
        .build()?;
    match cli.command {
        Command::Login { name } => login(&client, &server, &name).await,
        Command::Logout => logout(&client, &server).await,
        Command::Remind { at, message } => {
            action(
                &client,
                &server,
                "api/agent/actions/reminders",
                &ReminderRequest {
                    message: &message,
                    at: &at,
                },
            )
            .await
        }
        Command::WatchReply {
            email,
            label,
            for_minutes,
        } => {
            action(
                &client,
                &server,
                "api/agent/actions/reply-watches",
                &ReplyWatchRequest {
                    email: &email,
                    label: label.as_deref(),
                    expires_in_seconds: u32::from(for_minutes) * 60,
                },
            )
            .await
        }
        Command::Status => {
            match credential_entry(&server).get_password() {
                Ok(_) => println!("connected"),
                Err(keyring::Error::NoEntry) => println!("not connected"),
                Err(error) => {
                    return Err(error).context("could not access the OS credential store")
                }
            }
            Ok(())
        }
    }
}

async fn login(client: &Client, server: &Url, name: &str) -> Result<()> {
    if credential_entry(server).get_password().is_ok() {
        bail!("already connected; run `lightfriend logout` before pairing again");
    }
    let response = client
        .post(server.join("api/agent/pairing/start")?)
        .json(&StartPairingRequest { client_name: name })
        .send()
        .await
        .context("could not start pairing")?;
    if !response.status().is_success() {
        bail!("pairing was rejected ({})", response.status());
    }
    let pairing: StartPairingResponse =
        response.json().await.context("invalid pairing response")?;
    if pairing.status != "accepted" {
        bail!("pairing was rejected");
    }
    let verification_url = server.join(pairing.verification_path.trim_start_matches('/'))?;
    println!("Open {verification_url}");
    println!("Then open Settings > Connections > Webhooks & API & CLI > Local agent CLI.");
    println!("Enter pairing code: {}", pairing.user_code);
    println!("Never paste a Lightfriend token into an agent chat, prompt, or URL.");
    let _ = webbrowser::open(verification_url.as_str());

    let deadline = Instant::now() + Duration::from_secs(pairing.expires_in);
    while Instant::now() < deadline {
        tokio::time::sleep(Duration::from_secs(pairing.poll_interval.max(2))).await;
        let response = client
            .post(server.join("api/agent/pairing/poll")?)
            .json(&PollPairingRequest {
                device_code: &pairing.device_code,
            })
            .send()
            .await
            .context("pairing poll failed")?;
        if response.status() == StatusCode::ACCEPTED {
            continue;
        }
        if !response.status().is_success() {
            bail!("pairing expired or was rejected ({})", response.status());
        }
        let result: PollPairingResponse =
            response.json().await.context("invalid pairing response")?;
        if result.status != "accepted" {
            bail!("pairing was rejected");
        }
        let token = result
            .token
            .ok_or_else(|| anyhow!("pairing returned no credential"))?;
        credential_entry(server)
            .set_password(&token)
            .context("could not store the credential in the OS credential store")?;
        println!("connected; the credential is stored in your OS credential store");
        return Ok(());
    }
    bail!("pairing expired; run `lightfriend login` again")
}

async fn logout(client: &Client, server: &Url) -> Result<()> {
    let entry = credential_entry(server);
    let token = match entry.get_password() {
        Ok(token) => token,
        Err(keyring::Error::NoEntry) => {
            println!("not connected");
            return Ok(());
        }
        Err(error) => return Err(error).context("could not access the OS credential store"),
    };
    let response = client
        .delete(server.join("api/agent/credential")?)
        .bearer_auth(&token)
        .send()
        .await
        .context("could not revoke the server credential")?;
    if response.status() != StatusCode::NO_CONTENT && response.status() != StatusCode::UNAUTHORIZED
    {
        bail!(
            "server credential revocation failed ({})",
            response.status()
        );
    }
    entry
        .delete_credential()
        .context("credential was revoked but could not be removed from the OS credential store")?;
    println!("disconnected and revoked");
    Ok(())
}

async fn action<T: Serialize>(client: &Client, server: &Url, path: &str, body: &T) -> Result<()> {
    let token = credential_entry(server)
        .get_password()
        .context("not connected; run `lightfriend login` in your local terminal")?;
    let response = client
        .post(server.join(path)?)
        .bearer_auth(token)
        .header("Idempotency-Key", random_idempotency_key())
        .json(body)
        .send()
        .await
        .context("Lightfriend request failed")?;
    let status_code = response.status();
    let result = response.json::<ActionResponse>().await.ok();
    let status = result
        .as_ref()
        .map(|result| result.status.as_str())
        .unwrap_or("failed");
    if status_code.is_success() && status == "accepted" {
        println!("accepted");
        Ok(())
    } else {
        bail!(
            "{}",
            if status == "rejected" {
                "rejected"
            } else {
                "failed"
            }
        )
    }
}

fn validate_server(value: &str) -> Result<Url> {
    let mut url = Url::parse(value).context("invalid server URL")?;
    if url.username() != ""
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        bail!("server URL must not contain credentials, a query, or a fragment");
    }
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("server URL has no host"))?;
    let local = host == "localhost" || host == "127.0.0.1" || host == "::1";
    if url.scheme() != "https" && !(url.scheme() == "http" && local) {
        bail!("HTTPS is required except for localhost development");
    }
    url.set_path("/");
    Ok(url)
}

fn credential_entry(server: &Url) -> Entry {
    let account = format!(
        "{}:{}",
        server.host_str().unwrap_or("unknown"),
        server.port_or_known_default().unwrap_or(443)
    );
    Entry::new(KEYRING_SERVICE, &account).expect("valid keyring service and account")
}

fn random_idempotency_key() -> String {
    let mut bytes = [0_u8; 16];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
