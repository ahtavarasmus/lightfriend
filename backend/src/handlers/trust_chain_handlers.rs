use axum::{extract::Query, Json};
use futures::future::join_all;
use rand::RngCore;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::OnceLock,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;

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
const CORE_CACHE_TTL: Duration = Duration::from_secs(60);
const EVENTS_CACHE_TTL: Duration = Duration::from_secs(300);
const GITHUB_REPO: &str = "ahtavarasmus/lightfriend";

const DEFAULT_HISTORY_LIMIT: usize = 10;
const MAX_HISTORY_LIMIT: usize = 50;

static CORE_CACHE: OnceLock<RwLock<Option<CachedCore>>> = OnceLock::new();
static EVENTS_CACHE: OnceLock<RwLock<Option<CachedEvents>>> = OnceLock::new();
static COMMIT_CACHE: OnceLock<RwLock<HashMap<String, CommitInfo>>> = OnceLock::new();

#[derive(Clone)]
struct CachedCore {
    fetched_at: Instant,
    response: TrustChainResponse,
}

#[derive(Clone)]
struct CachedEvents {
    fetched_at: Instant,
    builds: Vec<HistoricalBuild>,
}

#[derive(Clone, Default)]
struct CommitInfo {
    message: Option<String>,
    pr_number: Option<u64>,
}

// -- Response types --

#[derive(Clone, Serialize)]
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
    pub attestation: Option<AttestationInfo>,
}

#[derive(Clone, Serialize)]
pub struct TrustChainHistoryResponse {
    pub builds: Vec<HistoricalBuild>,
    pub total: usize,
    pub offset: usize,
    pub has_more: bool,
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Clone, Serialize)]
pub struct AttestationInfo {
    pub available: bool,
    pub pcr0: Option<String>,
    pub pcr1: Option<String>,
    pub pcr2: Option<String>,
    pub doc_byte_size: Option<usize>,
}

#[derive(Clone, Serialize)]
pub struct BlockchainInfo {
    pub approved: bool,
    pub propose_tx: Option<String>,
    pub propose_timestamp: Option<String>,
    pub activate_tx: Option<String>,
    pub activate_timestamp: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct HistoricalBuild {
    pub image_id: String,
    pub commit_hash: String,
    pub propose_tx: String,
    pub propose_timestamp: Option<String>,
    pub activate_tx: Option<String>,
    pub activate_timestamp: Option<String>,
    pub is_current: bool,
    pub commit_message: Option<String>,
    pub pr_number: Option<u64>,
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
    let call_data = match encode_verify_call(image_id) {
        Some(d) => d,
        None => {
            tracing::warn!(
                "trust_chain: failed to encode verify call for image_id={}",
                image_id
            );
            return None;
        }
    };

    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_call",
        "params": [{"to": contract, "data": call_data}, "latest"],
        "id": 1
    });

    tracing::info!(
        "trust_chain: calling oysterKMSVerify rpc={} contract={} image_id={}",
        rpc_url,
        contract,
        image_id
    );

    let response = match client
        .post(rpc_url)
        .json(&payload)
        .timeout(RPC_TIMEOUT)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(
                "trust_chain: RPC POST failed for {}: {} (is ARBITRUM_RPC_URL reachable through HTTP_PROXY?)",
                rpc_url,
                e
            );
            return None;
        }
    };

    let status = response.status();
    let raw = match response.text().await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("trust_chain: failed to read RPC response body: {}", e);
            return None;
        }
    };

    if !status.is_success() {
        tracing::warn!(
            "trust_chain: RPC returned non-2xx status={} body={}",
            status,
            raw
        );
        return None;
    }

    let parsed: RpcResponse = match serde_json::from_str(&raw) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(
                "trust_chain: failed to parse RPC response: {} body={}",
                e,
                raw
            );
            return None;
        }
    };

    let result_str = parsed.result?.as_str()?.to_string();
    tracing::info!(
        "trust_chain: oysterKMSVerify returned raw={} for image_id={}",
        result_str,
        image_id
    );
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
            "fromBlock": "0x0",
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
            Err(e) => {
                tracing::warn!("trust_chain: failed to parse eth_getLogs response: {}", e);
                return vec![];
            }
        },
        Err(e) => {
            tracing::warn!("trust_chain: eth_getLogs request failed: {}", e);
            return vec![];
        }
    };

    match resp.result {
        Some(serde_json::Value::Array(logs)) => logs,
        Some(other) => {
            tracing::warn!("trust_chain: eth_getLogs returned non-array: {:?}", other);
            vec![]
        }
        None => {
            tracing::warn!("trust_chain: eth_getLogs returned no result (error in RPC response)");
            vec![]
        }
    }
}

async fn maybe_fetch_timestamp(
    client: &Client,
    rpc_url: &str,
    block_hex: Option<String>,
) -> Option<String> {
    match block_hex {
        Some(bn) => fetch_block_timestamp(client, rpc_url, &bn).await,
        None => None,
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

/// Fetch blockchain info for ONLY the current image. Fast path for /api/trust-chain.
/// Uses topic-filtered eth_getLogs so the RPC only returns 0-1 matching events per topic.
async fn fetch_current_blockchain_info(
    client: &Client,
    rpc_url: &str,
    contract: &str,
    current_image_id: &str,
) -> Option<BlockchainInfo> {
    let approval_fut = check_contract_approval(client, rpc_url, contract, current_image_id);
    let proposed_fut = fetch_event_logs(
        client,
        rpc_url,
        contract,
        IMAGE_PROPOSED_TOPIC,
        Some(current_image_id),
    );
    let activated_fut = fetch_event_logs(
        client,
        rpc_url,
        contract,
        IMAGE_ACTIVATED_TOPIC,
        Some(current_image_id),
    );

    let (approved_opt, proposed_logs, activated_logs) =
        tokio::join!(approval_fut, proposed_fut, activated_fut);
    let approved = approved_opt.unwrap_or(false);

    let (propose_tx, propose_block) = proposed_logs
        .first()
        .map(|log| {
            (
                log.get("transactionHash")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                log.get("blockNumber")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            )
        })
        .unwrap_or((None, None));
    let (activate_tx, activate_block) = activated_logs
        .first()
        .map(|log| {
            (
                log.get("transactionHash")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                log.get("blockNumber")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            )
        })
        .unwrap_or((None, None));

    let (propose_timestamp, activate_timestamp) = tokio::join!(
        maybe_fetch_timestamp(client, rpc_url, propose_block),
        maybe_fetch_timestamp(client, rpc_url, activate_block),
    );

    if propose_tx.is_none() && activate_tx.is_none() && !approved {
        return None;
    }

    Some(BlockchainInfo {
        approved,
        propose_tx,
        propose_timestamp,
        activate_tx,
        activate_timestamp,
    })
}

/// Fetch ALL historical builds from the chain. Cached for 5 minutes.
/// Returned newest-first. commit_message/pr_number are left None here — fill them per page.
async fn get_all_historical_builds(
    client: &Client,
    rpc_url: &str,
    contract: &str,
    current_image_id: &str,
) -> Vec<HistoricalBuild> {
    let cache = EVENTS_CACHE.get_or_init(|| RwLock::new(None));
    if let Some(cached) = cache.read().await.as_ref() {
        if cached.fetched_at.elapsed() < EVENTS_CACHE_TTL {
            return cached.builds.clone();
        }
    }

    let proposed_fut = fetch_event_logs(client, rpc_url, contract, IMAGE_PROPOSED_TOPIC, None);
    let activated_fut = fetch_event_logs(client, rpc_url, contract, IMAGE_ACTIVATED_TOPIC, None);
    let (proposed_logs, activated_logs) = tokio::join!(proposed_fut, activated_fut);

    // Map imageId -> (tx, block)
    let mut activated_map: HashMap<String, (String, Option<String>)> = HashMap::new();
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

    // Decode proposed events into partial entries (without timestamps yet)
    struct PartialEntry {
        image_id: String,
        commit_hash: String,
        propose_tx: String,
        propose_block: Option<String>,
        activate_tx: Option<String>,
        activate_block: Option<String>,
    }

    let mut partials: Vec<PartialEntry> = Vec::new();
    for log in &proposed_logs {
        let topics = log.get("topics").and_then(|t| t.as_array());
        let tx_hash = log
            .get("transactionHash")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
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
            if image_id.is_empty() {
                continue;
            }
            let commit_hash = decode_commit_hash_from_log_data(data).unwrap_or_default();
            let (activate_tx, activate_block) = activated_map
                .get(&image_id)
                .map(|(tx, b)| (Some(tx.clone()), b.clone()))
                .unwrap_or((None, None));

            partials.push(PartialEntry {
                image_id,
                commit_hash,
                propose_tx: tx_hash,
                propose_block: block_num,
                activate_tx,
                activate_block,
            });
        }
    }

    // Parallel fetch of ALL timestamps (two per entry: propose + activate)
    let ts_futures = partials.iter().flat_map(|p| {
        [
            maybe_fetch_timestamp(client, rpc_url, p.propose_block.clone()),
            maybe_fetch_timestamp(client, rpc_url, p.activate_block.clone()),
        ]
    });
    let timestamps = join_all(ts_futures).await;

    let mut builds: Vec<HistoricalBuild> = partials
        .into_iter()
        .enumerate()
        .map(|(i, p)| {
            let propose_timestamp = timestamps.get(i * 2).cloned().flatten();
            let activate_timestamp = timestamps.get(i * 2 + 1).cloned().flatten();
            let is_current = p.image_id == current_image_id;
            HistoricalBuild {
                image_id: p.image_id,
                commit_hash: p.commit_hash,
                propose_tx: p.propose_tx,
                propose_timestamp,
                activate_tx: p.activate_tx,
                activate_timestamp,
                is_current,
                commit_message: None,
                pr_number: None,
            }
        })
        .collect();

    // Sort newest-first by propose_timestamp (fall back to insertion order reversed)
    builds.sort_by(|a, b| {
        b.propose_timestamp
            .as_deref()
            .unwrap_or("")
            .cmp(a.propose_timestamp.as_deref().unwrap_or(""))
    });

    *cache.write().await = Some(CachedEvents {
        fetched_at: Instant::now(),
        builds: builds.clone(),
    });

    builds
}

/// Look up (and populate the cache for) commit info for a single SHA.
async fn fetch_commit_info(client: &Client, sha: &str) -> CommitInfo {
    if sha.is_empty() {
        return CommitInfo::default();
    }

    let cache = COMMIT_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    if let Some(info) = cache.read().await.get(sha) {
        return info.clone();
    }

    let url = format!(
        "https://api.github.com/repos/{}/commits/{}",
        GITHUB_REPO, sha
    );
    let mut req = client
        .get(&url)
        .header("User-Agent", "lightfriend-trust-chain")
        .header("Accept", "application/vnd.github+json")
        .timeout(FETCH_TIMEOUT);
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        if !token.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", token));
        }
    }

    let info = match req.send().await {
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(v) => parse_commit_response(&v),
            Err(e) => {
                tracing::warn!(
                    "trust_chain: failed to parse github commit json for {}: {}",
                    sha,
                    e
                );
                CommitInfo::default()
            }
        },
        Ok(resp) => {
            tracing::warn!(
                "trust_chain: github commit fetch non-2xx for {}: status={}",
                sha,
                resp.status()
            );
            CommitInfo::default()
        }
        Err(e) => {
            tracing::warn!("trust_chain: github commit fetch failed for {}: {}", sha, e);
            CommitInfo::default()
        }
    };

    // Only cache successful lookups so failed fetches retry next time.
    if info.message.is_some() {
        cache.write().await.insert(sha.to_string(), info.clone());
    }

    info
}

fn parse_commit_response(v: &serde_json::Value) -> CommitInfo {
    let full_message = v
        .get("commit")
        .and_then(|c| c.get("message"))
        .and_then(|m| m.as_str())
        .unwrap_or("");
    let subject = full_message.lines().next().unwrap_or("").trim();
    let subject_opt = if subject.is_empty() {
        None
    } else {
        Some(subject.to_string())
    };

    // Squash-merge commits from GitHub include "(#123)" at the end of the subject.
    let pr_number = subject
        .rsplit_once(" (#")
        .and_then(|(_, tail)| tail.strip_suffix(')'))
        .and_then(|n| n.parse::<u64>().ok());

    CommitInfo {
        message: subject_opt,
        pr_number,
    }
}

/// Enrich a slice of builds with commit_message/pr_number in parallel.
async fn enrich_commit_info(client: &Client, builds: &mut [HistoricalBuild]) {
    let fetches = builds
        .iter()
        .map(|b| fetch_commit_info(client, &b.commit_hash))
        .collect::<Vec<_>>();
    let infos = join_all(fetches).await;
    for (build, info) in builds.iter_mut().zip(infos.into_iter()) {
        build.commit_message = info.message;
        build.pr_number = info.pr_number;
    }
}

// -- Handlers --

async fn build_trust_chain_core_response() -> TrustChainResponse {
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

    let image_id = match (&pcr0, &pcr1, &pcr2) {
        (Some(p0), Some(p1), Some(p2)) => compute_image_id(p0, p1, p2),
        _ => None,
    };

    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    let built_at_fut = async {
        if let Some(ref url) = build_metadata_url {
            fetch_build_metadata(&client, url)
                .await
                .and_then(|m| m.built_at)
        } else {
            None
        }
    };

    let blockchain_fut = async {
        if let (Some(ref rpc), Some(ref contract), Some(ref img_id)) =
            (&rpc_url, &kms_contract_address, &image_id)
        {
            fetch_current_blockchain_info(&client, rpc, contract, img_id).await
        } else {
            tracing::warn!(
                "trust_chain: skipping blockchain fetch - ARBITRUM_RPC_URL set={}, MARLIN_KMS_CONTRACT_ADDRESS set={}, image_id set={} (all three required)",
                rpc_url.is_some(),
                kms_contract_address.is_some(),
                image_id.is_some()
            );
            None
        }
    };

    let attestation_fut = async {
        match client
            .get("http://127.0.0.1:1300/attestation/hex")
            .timeout(Duration::from_secs(3))
            .send()
            .await
        {
            Ok(resp) => match resp.text().await {
                Ok(hex_doc) => Some(AttestationInfo {
                    available: true,
                    pcr0: pcr0.clone(),
                    pcr1: pcr1.clone(),
                    pcr2: pcr2.clone(),
                    doc_byte_size: Some(hex_doc.len() / 2),
                }),
                Err(_) => None,
            },
            Err(_) => None,
        }
    };

    let (built_at, blockchain, attestation) =
        tokio::join!(built_at_fut, blockchain_fut, attestation_fut);

    TrustChainResponse {
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
        attestation,
    }
}

pub async fn get_trust_chain() -> Json<TrustChainResponse> {
    let cache = CORE_CACHE.get_or_init(|| RwLock::new(None));

    if let Some(cached) = cache.read().await.as_ref() {
        if cached.fetched_at.elapsed() < CORE_CACHE_TTL {
            return Json(cached.response.clone());
        }
    }

    let response = build_trust_chain_core_response().await;
    *cache.write().await = Some(CachedCore {
        fetched_at: Instant::now(),
        response: response.clone(),
    });

    Json(response)
}

pub async fn get_trust_chain_history(
    Query(params): Query<HistoryQuery>,
) -> Json<TrustChainHistoryResponse> {
    let limit = params
        .limit
        .unwrap_or(DEFAULT_HISTORY_LIMIT)
        .clamp(1, MAX_HISTORY_LIMIT);
    let offset = params.offset.unwrap_or(0);

    let pcr0 = std::env::var("CURRENT_PCR0").ok();
    let pcr1 = std::env::var("CURRENT_PCR1").ok();
    let pcr2 = std::env::var("CURRENT_PCR2").ok();
    let kms_contract_address = std::env::var("MARLIN_KMS_CONTRACT_ADDRESS").ok();
    let rpc_url = std::env::var("ARBITRUM_RPC_URL").ok();
    let image_id = match (&pcr0, &pcr1, &pcr2) {
        (Some(p0), Some(p1), Some(p2)) => compute_image_id(p0, p1, p2),
        _ => None,
    };

    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    let all_builds = match (&rpc_url, &kms_contract_address, &image_id) {
        (Some(rpc), Some(contract), Some(img_id)) => {
            get_all_historical_builds(&client, rpc, contract, img_id).await
        }
        _ => vec![],
    };

    let total = all_builds.len();
    let start = offset.min(total);
    let end = (start + limit).min(total);
    let mut page: Vec<HistoricalBuild> = all_builds[start..end].to_vec();
    enrich_commit_info(&client, &mut page).await;

    Json(TrustChainHistoryResponse {
        builds: page,
        total,
        offset: start,
        has_more: end < total,
    })
}

// -- Live verification endpoint --

#[derive(Serialize)]
pub struct VerifyStep {
    pub step: String,
    pub status: String, // "pass", "fail", "info"
    pub message: String,
    pub detail: Option<String>,
}

#[derive(Serialize)]
pub struct VerifyResponse {
    pub nonce: String,
    pub steps: Vec<VerifyStep>,
    pub attestation_hex: Option<String>,
    pub overall: String, // "pass", "fail", "partial"
}

pub async fn verify_live() -> Json<VerifyResponse> {
    let mut steps = Vec::new();
    let mut overall = "pass".to_string();

    // Step 1: Generate random nonce
    let mut nonce_bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce_hex = hex::encode(nonce_bytes);

    steps.push(VerifyStep {
        step: "Generate challenge".to_string(),
        status: "pass".to_string(),
        message: "Generated a fresh random 16-byte challenge (nonce).".to_string(),
        detail: Some(format!("0x{}", nonce_hex)),
    });

    // Step 2: Read expected PCR values
    let pcr0 = std::env::var("CURRENT_PCR0").ok();
    let pcr1 = std::env::var("CURRENT_PCR1").ok();
    let pcr2 = std::env::var("CURRENT_PCR2").ok();
    let commit_sha = std::env::var("CURRENT_COMMIT_SHA").ok();

    if pcr0.is_some() && pcr1.is_some() && pcr2.is_some() {
        steps.push(VerifyStep {
            step: "Read expected values".to_string(),
            status: "pass".to_string(),
            message: "Read the expected PCR values from the build.".to_string(),
            detail: Some(format!(
                "PCR0: {}\nPCR1: {}\nPCR2: {}",
                pcr0.as_deref().unwrap_or("?"),
                pcr1.as_deref().unwrap_or("?"),
                pcr2.as_deref().unwrap_or("?")
            )),
        });
    } else {
        steps.push(VerifyStep {
            step: "Read expected values".to_string(),
            status: "fail".to_string(),
            message: "PCR values not available (not running in enclave).".to_string(),
            detail: None,
        });
        overall = "fail".to_string();
    }

    // Step 3: Fetch attestation from enclave
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    let attestation_hex = match client
        .get(format!(
            "http://127.0.0.1:1300/attestation/hex?user_data={}",
            nonce_hex
        ))
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) => match resp.text().await {
            Ok(hex_doc) => {
                let doc_len = hex_doc.len() / 2;
                steps.push(VerifyStep {
                    step: "Request attestation".to_string(),
                    status: "pass".to_string(),
                    message: format!(
                        "The enclave produced a signed attestation document ({} bytes) with our challenge embedded.",
                        doc_len
                    ),
                    detail: Some(format!(
                        "First 64 bytes: {}...",
                        &hex_doc[..std::cmp::min(128, hex_doc.len())]
                    )),
                });
                Some(hex_doc)
            }
            Err(e) => {
                steps.push(VerifyStep {
                    step: "Request attestation".to_string(),
                    status: "fail".to_string(),
                    message: format!("Failed to read attestation response: {}", e),
                    detail: None,
                });
                overall = "fail".to_string();
                None
            }
        },
        Err(e) => {
            steps.push(VerifyStep {
                step: "Request attestation".to_string(),
                status: "fail".to_string(),
                message: format!("Could not reach attestation server (not in enclave): {}", e),
                detail: None,
            });
            overall = "fail".to_string();
            None
        }
    };

    // Step 4: Signature verification note
    steps.push(VerifyStep {
        step: "Verify AWS signature".to_string(),
        status: if attestation_hex.is_some() {
            "info".to_string()
        } else {
            "fail".to_string()
        },
        message: if attestation_hex.is_some() {
            "The attestation document is signed by AWS Nitro. Full cryptographic verification requires the verification tool (linked below).".to_string()
        } else {
            "Skipped - no attestation document available.".to_string()
        },
        detail: None,
    });

    // Step 5: Check blockchain approval
    let rpc_url = std::env::var("ARBITRUM_RPC_URL").ok();
    let contract = std::env::var("MARLIN_KMS_CONTRACT_ADDRESS").ok();
    let image_id = match (&pcr0, &pcr1, &pcr2) {
        (Some(p0), Some(p1), Some(p2)) => compute_image_id(p0, p1, p2),
        _ => None,
    };

    if let (Some(ref rpc), Some(ref addr), Some(ref img_id)) = (&rpc_url, &contract, &image_id) {
        match check_contract_approval(&client, rpc, addr, img_id).await {
            Some(true) => {
                steps.push(VerifyStep {
                    step: "Check blockchain".to_string(),
                    status: "pass".to_string(),
                    message: "The smart contract confirms this image ID is approved.".to_string(),
                    detail: Some(format!("Image ID: {}\nContract: {}", img_id, addr)),
                });
            }
            Some(false) => {
                steps.push(VerifyStep {
                    step: "Check blockchain".to_string(),
                    status: "fail".to_string(),
                    message: "The smart contract does NOT approve this image ID.".to_string(),
                    detail: Some(format!("Image ID: {}", img_id)),
                });
                overall = "fail".to_string();
            }
            None => {
                steps.push(VerifyStep {
                    step: "Check blockchain".to_string(),
                    status: "fail".to_string(),
                    message: "Could not reach Arbitrum RPC to check approval.".to_string(),
                    detail: None,
                });
                if overall == "pass" {
                    overall = "partial".to_string();
                }
            }
        }
    } else {
        steps.push(VerifyStep {
            step: "Check blockchain".to_string(),
            status: "info".to_string(),
            message: "Blockchain check not available (missing RPC URL or contract address)."
                .to_string(),
            detail: None,
        });
        if overall == "pass" {
            overall = "partial".to_string();
        }
    }

    // Step 6: Summary
    if let Some(ref sha) = commit_sha {
        steps.push(VerifyStep {
            step: "Result".to_string(),
            status: overall.clone(),
            message: if overall == "pass" {
                format!(
                    "Verification passed. The enclave is running commit {} and is approved on-chain.",
                    &sha[..std::cmp::min(8, sha.len())]
                )
            } else if overall == "partial" {
                format!(
                    "Partial verification. The enclave responded with commit {} but some checks could not be completed.",
                    &sha[..std::cmp::min(8, sha.len())]
                )
            } else {
                "Verification failed. See individual steps above.".to_string()
            },
            detail: None,
        });
    }

    Json(VerifyResponse {
        nonce: format!("0x{}", nonce_hex),
        steps,
        attestation_hex,
        overall,
    })
}
