use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use oyster_sdk::attestation::{self, AttestationExpectations, AWS_ROOT_KEY};
use rand::RngCore;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use sha3::{Digest, Keccak256};
use std::time::{SystemTime, UNIX_EPOCH};
use url::Url;

#[derive(Parser, Debug)]
struct Args {
    /// Lightfriend base URL, for example https://lightfriend.ai
    target: String,

    /// Optional public CI metadata URL to compare against
    #[arg(long)]
    build_metadata_url: Option<String>,

    /// Optional JSON-RPC URL for checking oysterKMSVerify on-chain
    #[arg(long, env = "ARBITRUM_RPC_URL")]
    rpc_url: Option<String>,

    /// Maximum allowed attestation age in milliseconds
    #[arg(long, default_value_t = 300_000)]
    max_age_ms: u64,
}

#[derive(Debug, Deserialize)]
struct LiveMetadata {
    attestation_raw_url: String,
    attestation_hex_url: String,
    build_metadata_url: Option<String>,
    commit_sha: Option<String>,
    workflow_run_id: Option<String>,
    image_ref: Option<String>,
    eif_sha256: Option<String>,
    pcr0: Option<String>,
    pcr1: Option<String>,
    pcr2: Option<String>,
    kms_contract_address: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BuildMetadata {
    commit_sha: Option<String>,
    image_ref: Option<String>,
    eif_key: Option<String>,
    eif_sha256: Option<String>,
    pcr0: Option<String>,
    pcr1: Option<String>,
    pcr2: Option<String>,
    workflow_run_id: Option<String>,
}

fn normalize_base_url(target: &str) -> Result<Url> {
    let with_scheme = if target.starts_with("http://") || target.starts_with("https://") {
        target.to_string()
    } else {
        format!("https://{target}")
    };
    Url::parse(&with_scheme).context("failed to parse target URL")
}

fn parse_pcr(value: &str) -> Result<[u8; 48]> {
    let clean = value.strip_prefix("0x").unwrap_or(value);
    let bytes = hex::decode(clean).with_context(|| format!("invalid PCR hex: {value}"))?;
    if bytes.len() != 48 {
        bail!(
            "PCR must decode to 48 bytes, got {} for {}",
            bytes.len(),
            value
        );
    }
    let mut out = [0u8; 48];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn parse_bytes32(value: &str) -> Result<[u8; 32]> {
    let clean = value.strip_prefix("0x").unwrap_or(value);
    let bytes = hex::decode(clean).with_context(|| format!("invalid bytes32 hex: {value}"))?;
    if bytes.len() != 32 {
        bail!(
            "bytes32 value must decode to 32 bytes, got {} for {}",
            bytes.len(),
            value
        );
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn bytes_to_hex_prefixed(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

fn selector(signature: &str) -> [u8; 4] {
    let mut hasher = Keccak256::new();
    hasher.update(signature.as_bytes());
    let hash = hasher.finalize();
    [hash[0], hash[1], hash[2], hash[3]]
}

fn encode_bool_call(signature: &str, bytes32_arg_hex: &str) -> Result<String> {
    let arg = parse_bytes32(bytes32_arg_hex)?;
    let mut data = Vec::with_capacity(4 + 32);
    data.extend_from_slice(&selector(signature));
    data.extend_from_slice(&arg);
    Ok(format!("0x{}", hex::encode(data)))
}

async fn fetch_json<T: for<'de> Deserialize<'de>>(client: &Client, url: Url) -> Result<T> {
    let resp = client.get(url).send().await?.error_for_status()?;
    Ok(resp.json::<T>().await?)
}

async fn fetch_attestation(
    client: &Client,
    metadata: &LiveMetadata,
    challenge_hex: &str,
) -> Result<Vec<u8>> {
    let mut url = Url::parse(&metadata.attestation_raw_url)
        .context("invalid attestation_raw_url from metadata endpoint")?;
    url.query_pairs_mut()
        .append_pair("user_data", challenge_hex);
    let resp = client.get(url).send().await?.error_for_status()?;
    Ok(resp.bytes().await?.to_vec())
}

async fn check_contract(client: &Client, rpc_url: &str, contract: &str, pcr0: &str) -> Result<()> {
    let call_data = encode_bool_call("oysterKMSVerify(bytes32)", pcr0)?;
    let payload = json!({
        "jsonrpc": "2.0",
        "method": "eth_call",
        "params": [
            {
                "to": contract,
                "data": call_data
            },
            "latest"
        ],
        "id": 1
    });

    let value = client
        .post(rpc_url)
        .json(&payload)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    if let Some(err) = value.get("error") {
        bail!("rpc error checking contract approval: {err}");
    }

    let result = value
        .get("result")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing eth_call result"))?;

    let approved = result.ends_with('1');
    if !approved {
        bail!("contract does not approve pcr0 {pcr0}");
    }

    Ok(())
}

fn compare_optional(label: &str, live: &Option<String>, build: &Option<String>) -> Result<()> {
    if let (Some(live), Some(build)) = (live, build) {
        if live != build {
            bail!("{label} mismatch between live metadata and build metadata");
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let base_url = normalize_base_url(&args.target)?;
    let client = Client::builder().build()?;

    let metadata_url = base_url.join("/.well-known/lightfriend/attestation")?;
    let live = fetch_json::<LiveMetadata>(&client, metadata_url).await?;

    let pcr0 = live
        .pcr0
        .as_deref()
        .ok_or_else(|| anyhow!("live metadata missing pcr0"))?;
    let pcr1 = live
        .pcr1
        .as_deref()
        .ok_or_else(|| anyhow!("live metadata missing pcr1"))?;
    let pcr2 = live
        .pcr2
        .as_deref()
        .ok_or_else(|| anyhow!("live metadata missing pcr2"))?;

    let build_metadata_url = args
        .build_metadata_url
        .as_deref()
        .map(str::to_owned)
        .or_else(|| live.build_metadata_url.clone());

    if let Some(build_metadata_url) = build_metadata_url.as_deref() {
        let build = fetch_json::<BuildMetadata>(&client, Url::parse(build_metadata_url)?).await?;
        compare_optional("commit_sha", &live.commit_sha, &build.commit_sha)?;
        compare_optional(
            "workflow_run_id",
            &live.workflow_run_id,
            &build.workflow_run_id,
        )?;
        compare_optional("image_ref", &live.image_ref, &build.image_ref)?;
        compare_optional("eif_sha256", &live.eif_sha256, &build.eif_sha256)?;
        compare_optional("pcr0", &live.pcr0, &build.pcr0)?;
        compare_optional("pcr1", &live.pcr1, &build.pcr1)?;
        compare_optional("pcr2", &live.pcr2, &build.pcr2)?;
        if let Some(eif_key) = build.eif_key {
            println!("Build metadata EIF key: {eif_key}");
        }
    }

    let mut challenge = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut challenge);
    let challenge_hex = hex::encode(challenge);
    let attestation_doc = fetch_attestation(&client, &live, &challenge_hex).await?;

    let now_ms = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
    let decoded = attestation::verify(
        &attestation_doc,
        AttestationExpectations {
            root_public_key: Some(&AWS_ROOT_KEY),
            age_ms: Some((args.max_age_ms, now_ms)),
            user_data: Some(&challenge),
            ..Default::default()
        },
    )
    .context("attestation verification failed")?;

    let expected_pcr0 = parse_pcr(pcr0)?;
    let expected_pcr1 = parse_pcr(pcr1)?;
    let expected_pcr2 = parse_pcr(pcr2)?;
    if decoded.pcrs[0] != expected_pcr0 {
        bail!("attested PCR0 does not match live metadata");
    }
    if decoded.pcrs[1] != expected_pcr1 {
        bail!("attested PCR1 does not match live metadata");
    }
    if decoded.pcrs[2] != expected_pcr2 {
        bail!("attested PCR2 does not match live metadata");
    }

    if let (Some(rpc_url), Some(contract)) = (
        args.rpc_url.as_deref(),
        live.kms_contract_address.as_deref(),
    ) {
        check_contract(&client, rpc_url, contract, pcr0).await?;
        println!("Contract approval verified: {contract}");
    } else {
        println!("Contract approval check skipped");
    }

    println!("Verification succeeded");
    println!(
        "Commit: {}",
        live.commit_sha.unwrap_or_else(|| "unknown".to_string())
    );
    println!(
        "Workflow run: {}",
        live.workflow_run_id
            .unwrap_or_else(|| "unknown".to_string())
    );
    println!(
        "Image: {}",
        live.image_ref.unwrap_or_else(|| "unknown".to_string())
    );
    println!(
        "EIF sha256: {}",
        live.eif_sha256.unwrap_or_else(|| "unknown".to_string())
    );
    println!("PCR0: {}", bytes_to_hex_prefixed(&decoded.pcrs[0]));
    println!("PCR1: {}", bytes_to_hex_prefixed(&decoded.pcrs[1]));
    println!("PCR2: {}", bytes_to_hex_prefixed(&decoded.pcrs[2]));
    println!("Image ID: 0x{}", hex::encode(decoded.image_id));
    println!("Enclave key: 0x{}", hex::encode(decoded.public_key));
    println!("Fresh challenge (user_data): 0x{}", challenge_hex);

    Ok(())
}
