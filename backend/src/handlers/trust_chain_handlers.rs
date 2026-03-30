use axum::Json;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;

// Hardcoded Keccak-256 values for Solidity ABI encoding.
// These are constants derived from the contract's function/event signatures.

/// keccak256("oysterKMSVerify(bytes32)") first 4 bytes
const OYSTER_KMS_VERIFY_SELECTOR: [u8; 4] = [0x1f, 0xe3, 0x65, 0xfa];

/// keccak256("ImageProposed(bytes32,string,uint256)")
const IMAGE_PROPOSED_TOPIC: &str =
    "0x97878f3f42042ec2f400b9e9ae0f231760829b25910474c62dbbee1adfb5dcfc";

/// keccak256("ImageActivated(bytes32)")
const IMAGE_ACTIVATED_TOPIC: &str =
    "0x4f887831a5a2a01a8f0fbffb2dcc0b047560a93fda3377e25b2ce8228455cc0e";

const RPC_TIMEOUT: Duration = Duration::from_secs(5);
const FETCH_TIMEOUT: Duration = Duration::from_secs(5);

// -- Response types --

#[derive(Serialize)]
pub struct TrustChainResponse {
    pub commit_sha: Option<String>,
    pub workflow_run_id: Option<String>,
    pub image_ref: Option<String>,
    pub eif_sha256: Option<String>,
    pub pcr0: Option<String>,
    pub pcr1: Option<String>,
    pub pcr2: Option<String>,
    pub image_id: Option<String>,
    pub kms_contract_address: Option<String>,
    pub built_at: Option<String>,
    pub build_metadata_url: Option<String>,
    pub blockchain: Option<BlockchainInfo>,
    pub history: Vec<HistoricalBuild>,
}

#[derive(Serialize)]
pub struct BlockchainInfo {
    pub approved: bool,
    pub propose_tx: Option<String>,
    pub propose_timestamp: Option<String>,
    pub activate_tx: Option<String>,
    pub activate_timestamp: Option<String>,
}

#[derive(Serialize)]
pub struct HistoricalBuild {
    pub image_id: String,
    pub commit_hash: String,
    pub propose_tx: String,
    pub propose_timestamp: Option<String>,
    pub activate_tx: Option<String>,
    pub activate_timestamp: Option<String>,
    pub is_current: bool,
}

#[derive(Deserialize)]
struct BuildMetadata {
    built_at: Option<String>,
}

#[derive(Deserialize)]
struct RpcResponse {
    result: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct BlockResponse {
    result: Option<BlockResult>,
}

#[derive(Deserialize)]
struct BlockResult {
    timestamp: Option<String>,
}

// -- Helpers --

fn parse_pcr_bytes(hex_str: &str) -> Option<Vec<u8>> {
    let clean = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    hex::decode(clean).ok()
}

fn compute_image_id(pcr0: &str, pcr1: &str, pcr2: &str) -> Option<String> {
    let pcr0_bytes = parse_pcr_bytes(pcr0)?;
    let pcr1_bytes = parse_pcr_bytes(pcr1)?;
    let pcr2_bytes = parse_pcr_bytes(pcr2)?;

    if pcr0_bytes.len() != 48 || pcr1_bytes.len() != 48 || pcr2_bytes.len() != 48 {
        return None;
    }

    // Matches the CI pipeline computation:
    // SHA256(bitflags_u32_be(0x00010007) || PCR0[48] || PCR1[48] || PCR2[48] || PCR16_zeros[48])
    let bitflags: [u8; 4] = [0x00, 0x01, 0x00, 0x07];
    let pcr16_zeros = [0u8; 48];

    let mut hasher = Sha256::new();
    hasher.update(bitflags);
    hasher.update(&pcr0_bytes);
    hasher.update(&pcr1_bytes);
    hasher.update(&pcr2_bytes);
    hasher.update(pcr16_zeros);

    let hash = hasher.finalize();
    Some(format!("0x{}", hex::encode(hash)))
}

fn encode_verify_call(image_id_hex: &str) -> Option<String> {
    let clean = image_id_hex.strip_prefix("0x").unwrap_or(image_id_hex);
    let id_bytes = hex::decode(clean).ok()?;
    if id_bytes.len() != 32 {
        return None;
    }

    let mut data = Vec::with_capacity(4 + 32);
    data.extend_from_slice(&OYSTER_KMS_VERIFY_SELECTOR);
    data.extend_from_slice(&id_bytes);
    Some(format!("0x{}", hex::encode(data)))
}

async fn fetch_build_metadata(client: &Client, url: &str) -> Option<BuildMetadata> {
    client
        .get(url)
        .timeout(FETCH_TIMEOUT)
        .send()
        .await
        .ok()?
        .json::<BuildMetadata>()
        .await
        .ok()
}

async fn check_contract_approval(
    client: &Client,
    rpc_url: &str,
    contract: &str,
    image_id: &str,
) -> Option<bool> {
    let call_data = encode_verify_call(image_id)?;
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_call",
        "params": [{"to": contract, "data": call_data}, "latest"],
        "id": 1
    });

    let resp: RpcResponse = client
        .post(rpc_url)
        .json(&payload)
        .timeout(RPC_TIMEOUT)
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    let result_str = resp.result?.as_str()?.to_string();
    Some(result_str.ends_with('1'))
}

async fn fetch_event_logs(
    client: &Client,
    rpc_url: &str,
    contract: &str,
    topic: &str,
    image_id_topic: Option<&str>,
) -> Vec<serde_json::Value> {
    let topics: Vec<serde_json::Value> = if let Some(id_topic) = image_id_topic {
        vec![serde_json::json!(topic), serde_json::json!(id_topic)]
    } else {
        vec![serde_json::json!(topic)]
    };

    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_getLogs",
        "params": [{
            "address": contract,
            "topics": topics,
            "fromBlock": "earliest",
            "toBlock": "latest"
        }],
        "id": 1
    });

    let resp: RpcResponse = match client
        .post(rpc_url)
        .json(&payload)
        .timeout(RPC_TIMEOUT)
        .send()
        .await
    {
        Ok(r) => match r.json().await {
            Ok(v) => v,
            Err(_) => return vec![],
        },
        Err(_) => return vec![],
    };

    match resp.result {
        Some(serde_json::Value::Array(logs)) => logs,
        _ => vec![],
    }
}

async fn fetch_block_timestamp(client: &Client, rpc_url: &str, block_hex: &str) -> Option<String> {
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_getBlockByNumber",
        "params": [block_hex, false],
        "id": 1
    });

    let resp: BlockResponse = client
        .post(rpc_url)
        .json(&payload)
        .timeout(RPC_TIMEOUT)
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    let ts_hex = resp.result?.timestamp?;
    let ts_clean = ts_hex.strip_prefix("0x").unwrap_or(&ts_hex);
    let ts_secs = u64::from_str_radix(ts_clean, 16).ok()?;
    let dt = chrono::DateTime::from_timestamp(ts_secs as i64, 0)?;
    Some(dt.to_rfc3339())
}

fn decode_commit_hash_from_log_data(data_hex: &str) -> Option<String> {
    // ABI-encoded data for ImageProposed: (string commitHash, uint256 activatesAt)
    // Layout: offset(32) + activatesAt(32) + string_length(32) + string_data(padded)
    let clean = data_hex.strip_prefix("0x").unwrap_or(data_hex);
    let bytes = hex::decode(clean).ok()?;
    if bytes.len() < 96 {
        return None;
    }

    // First 32 bytes: offset to string data
    // Skip to where the string length is
    let offset_bytes: [u8; 32] = bytes[0..32].try_into().ok()?;
    let offset = u256_to_usize(&offset_bytes)?;

    if bytes.len() < offset + 64 {
        return None;
    }

    // At offset: string length (32 bytes) + string data
    let len_bytes: [u8; 32] = bytes[offset..offset + 32].try_into().ok()?;
    let str_len = u256_to_usize(&len_bytes)?;

    if bytes.len() < offset + 32 + str_len {
        return None;
    }

    String::from_utf8(bytes[offset + 32..offset + 32 + str_len].to_vec()).ok()
}

fn u256_to_usize(bytes: &[u8; 32]) -> Option<usize> {
    // Only care about last 8 bytes for reasonable sizes
    let mut arr = [0u8; 8];
    arr.copy_from_slice(&bytes[24..32]);
    Some(u64::from_be_bytes(arr) as usize)
}

async fn fetch_blockchain_data(
    client: &Client,
    rpc_url: &str,
    contract: &str,
    current_image_id: &str,
) -> (Option<BlockchainInfo>, Vec<HistoricalBuild>) {
    // Check if current image is approved
    let approved = check_contract_approval(client, rpc_url, contract, current_image_id)
        .await
        .unwrap_or(false);

    // Fetch all ImageProposed events
    let proposed_logs =
        fetch_event_logs(client, rpc_url, contract, IMAGE_PROPOSED_TOPIC, None).await;

    // Fetch all ImageActivated events
    let activated_logs =
        fetch_event_logs(client, rpc_url, contract, IMAGE_ACTIVATED_TOPIC, None).await;

    // Build a map of imageId -> activation info
    let mut activated_map: std::collections::HashMap<String, (String, Option<String>)> =
        std::collections::HashMap::new();
    for log in &activated_logs {
        let topics = log.get("topics").and_then(|t| t.as_array());
        let tx_hash = log
            .get("transactionHash")
            .and_then(|v| v.as_str())
            .map(String::from);
        let block = log
            .get("blockNumber")
            .and_then(|v| v.as_str())
            .map(String::from);

        if let (Some(topics), Some(tx)) = (topics, tx_hash) {
            if let Some(image_id_val) = topics.get(1).and_then(|v| v.as_str()) {
                activated_map.insert(image_id_val.to_string(), (tx, block));
            }
        }
    }

    // Build history from proposed events
    let mut history = Vec::new();
    let mut current_blockchain = None;

    for log in &proposed_logs {
        let topics = log.get("topics").and_then(|t| t.as_array());
        let tx_hash = log
            .get("transactionHash")
            .and_then(|v| v.as_str())
            .map(String::from);
        let block_num = log
            .get("blockNumber")
            .and_then(|v| v.as_str())
            .map(String::from);
        let data = log.get("data").and_then(|v| v.as_str()).unwrap_or_default();

        if let Some(topics) = topics {
            let image_id = topics
                .get(1)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let commit_hash = decode_commit_hash_from_log_data(data).unwrap_or_default();

            let propose_tx = tx_hash.clone().unwrap_or_default();
            let propose_timestamp = if let Some(ref bn) = block_num {
                fetch_block_timestamp(client, rpc_url, bn).await
            } else {
                None
            };

            let (activate_tx, activate_timestamp) =
                if let Some((atx, ablock)) = activated_map.get(&image_id) {
                    let ats = if let Some(ref abn) = ablock {
                        fetch_block_timestamp(client, rpc_url, abn).await
                    } else {
                        None
                    };
                    (Some(atx.clone()), ats)
                } else {
                    (None, None)
                };

            let is_current = image_id == current_image_id;

            if is_current {
                current_blockchain = Some(BlockchainInfo {
                    approved,
                    propose_tx: Some(propose_tx.clone()),
                    propose_timestamp: propose_timestamp.clone(),
                    activate_tx: activate_tx.clone(),
                    activate_timestamp: activate_timestamp.clone(),
                });
            }

            history.push(HistoricalBuild {
                image_id,
                commit_hash,
                propose_tx,
                propose_timestamp,
                activate_tx,
                activate_timestamp,
                is_current,
            });
        }
    }

    // Reverse so newest is first
    history.reverse();

    // If we have approval but no matching proposed event, create minimal blockchain info
    if current_blockchain.is_none() && approved {
        current_blockchain = Some(BlockchainInfo {
            approved: true,
            propose_tx: None,
            propose_timestamp: None,
            activate_tx: None,
            activate_timestamp: None,
        });
    }

    (current_blockchain, history)
}

// -- Handler --

pub async fn get_trust_chain() -> Json<TrustChainResponse> {
    let commit_sha = std::env::var("CURRENT_COMMIT_SHA").ok();
    let workflow_run_id = std::env::var("CURRENT_WORKFLOW_RUN_ID").ok();
    let image_ref = std::env::var("CURRENT_IMAGE_REF").ok();
    let eif_sha256 = std::env::var("CURRENT_EIF_SHA256").ok();
    let pcr0 = std::env::var("CURRENT_PCR0").ok();
    let pcr1 = std::env::var("CURRENT_PCR1").ok();
    let pcr2 = std::env::var("CURRENT_PCR2").ok();
    let kms_contract_address = std::env::var("MARLIN_KMS_CONTRACT_ADDRESS").ok();
    let build_metadata_url = std::env::var("CURRENT_BUILD_METADATA_URL").ok();
    let rpc_url = std::env::var("ARBITRUM_RPC_URL").ok();

    // Compute image_id from PCRs
    let image_id = match (&pcr0, &pcr1, &pcr2) {
        (Some(p0), Some(p1), Some(p2)) => compute_image_id(p0, p1, p2),
        _ => None,
    };

    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    // Fetch built_at from gh-pages metadata
    let built_at = if let Some(ref url) = build_metadata_url {
        fetch_build_metadata(&client, url)
            .await
            .and_then(|m| m.built_at)
    } else {
        None
    };

    // Fetch blockchain data if we have RPC URL, contract, and image_id
    let (blockchain, history) = if let (Some(ref rpc), Some(ref contract), Some(ref img_id)) =
        (&rpc_url, &kms_contract_address, &image_id)
    {
        fetch_blockchain_data(&client, rpc, contract, img_id).await
    } else {
        (None, vec![])
    };

    Json(TrustChainResponse {
        commit_sha,
        workflow_run_id,
        image_ref,
        eif_sha256,
        pcr0,
        pcr1,
        pcr2,
        image_id,
        kms_contract_address,
        built_at,
        build_metadata_url,
        blockchain,
        history,
    })
}
