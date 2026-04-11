//! Bitcoin Core JSON-RPC client.
//!
//! Thin wrapper over reqwest that handles HTTP Basic authentication and the
//! JSON-RPC 1.0 protocol used by Bitcoin Core. The primary data extraction
//! method is [`BitcoinRpc::get_block`] which fetches a block at verbosity=2
//! (full transaction data) and computes all metrics in a single pass:
//!
//! - Block metadata (size, weight, difficulty, version)
//! - Fee statistics (total fees, median fee, median/p10/p90 fee rates in sat/vB)
//! - OP_RETURN classification (Runes, Omni, Counterparty, generic data carriers)
//! - Output script type counts (P2PKH, P2SH, P2WPKH, P2WSH, P2TR, multisig, etc.)
//! - Miner identification (from coinbase scriptSig and OP_RETURN outputs)
//! - BIP-54 signaling (coinbase nLockTime and nSequence)
//! - Taproot key-path vs script-path spend detection
//! - Ordinals inscription and BRC-20 detection in witness data
//! - Stamps protocol detection (bare multisig with fake pubkeys)
//!
//! Also provides methods for mempool queries, network stats, UTXO set info,
//! and price data from external APIs.

use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

use super::classifier::{self, OpReturnType};
use super::error::StatsError;

/// Calculate the encoded size of a Bitcoin CompactSize (varint) for a given value.
/// Used to accurately count witness byte overhead.
fn varint_size(n: u64) -> u64 {
    if n < 0xFD {
        1
    } else if n <= 0xFFFF {
        3
    } else if n <= 0xFFFF_FFFF {
        5
    } else {
        9
    }
}

/// Bitcoin Core JSON-RPC client. Holds a persistent HTTP client with
/// connection pooling and configurable timeouts.
pub struct BitcoinRpc {
    client: Client,
    url: String,
    user: String,
    password: String,
}

/// Response from `getblockchaininfo` RPC.
#[derive(Debug, Deserialize, serde::Serialize)]
pub struct BlockchainInfo {
    pub blocks: u64,
    pub chain: String,
    pub difficulty: f64,
    #[serde(rename = "verificationprogress")]
    pub verification_progress: f64,
    pub size_on_disk: u64,
    pub bestblockhash: String,
    #[serde(default)]
    pub time: u64,
}

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct MempoolInfo {
    pub size: u64,
    pub bytes: u64,
    pub usage: u64,
    #[serde(default)]
    pub total_fee: f64,
    pub maxmempool: u64,
    pub mempoolminfee: f64,
}

/// Response from `gettxoutsetinfo` RPC. Used to get the total UTXO count.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct TxoutSetInfo {
    /// Total number of unspent transaction outputs.
    pub txouts: u64,
    /// Total value of all UTXOs in BTC.
    pub total_amount: f64,
}

/// BTC/USD price from mempool.space `/api/v1/prices` endpoint.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct PriceInfo {
    #[serde(rename = "USD")]
    pub usd: f64,
    pub time: u64,
}

/// Fee and size data for a single mempool transaction from `getmempoolentry` RPC.
#[derive(Debug)]
pub struct MempoolEntryInfo {
    /// Transaction fee in satoshis.
    pub fee: u64,
    /// Virtual size in vbytes.
    pub vsize: u32,
}

/// Block metadata and transaction ID list from `getblock` verbosity=1.
/// Used by ZMQ block handler for fast block processing without full tx data.
#[derive(Debug)]
pub struct BlockTxids {
    pub height: u64,
    pub timestamp: u64,
    /// Block size in bytes (0 when fetched via getblockheader).
    pub size: u64,
    /// Block weight in weight units (0 when fetched via getblockheader).
    pub weight: u64,
    pub tx_count: u64,
    /// All transaction IDs in the block (empty when fetched via getblockheader).
    pub txids: Vec<String>,
}

/// Fully parsed block data from `getblock` verbosity=2.
/// Contains all metrics computed in a single pass over the block's transactions.
#[derive(Debug)]
pub struct Block {
    pub hash: String,
    pub height: u64,
    pub time: u64,
    pub n_tx: u64,
    pub size: u64,
    pub weight: u64,
    pub difficulty: f64,
    pub op_return_count: u64,
    pub op_return_bytes: u64,
    pub runes_count: u64,
    pub runes_bytes: u64,
    pub omni_count: u64,
    pub omni_bytes: u64,
    pub counterparty_count: u64,
    pub counterparty_bytes: u64,
    pub data_carrier_count: u64,
    pub data_carrier_bytes: u64,
    pub version: u32,
    pub total_fees: u64,
    pub median_fee: u64,
    pub median_fee_rate: f64,
    pub coinbase_locktime: u64,
    pub coinbase_sequence: u64,
    pub miner: String,
    pub segwit_spend_count: u64,
    /// Taproot v1 witness outputs created (not spends). Named _spend_ for historical reasons.
    pub taproot_spend_count: u64,
    // Output script type counts
    pub p2pk_count: u64,
    pub p2pkh_count: u64,
    pub p2sh_count: u64,
    pub p2wpkh_count: u64,
    pub p2wsh_count: u64,
    pub p2tr_count: u64,
    pub multisig_count: u64,
    pub unknown_script_count: u64,
    // Taproot spend type counts (inputs spending P2TR outputs)
    pub taproot_keypath_count: u64,
    pub taproot_scriptpath_count: u64,
    // Transaction-level metrics
    pub input_count: u64,
    pub output_count: u64,
    pub rbf_count: u64,
    // Size breakdown
    pub witness_bytes: u64,
    // Ordinals inscriptions
    pub inscription_count: u64,
    pub inscription_bytes: u64,
    pub brc20_count: u64,
    // Total value of non-coinbase outputs (satoshis)
    pub total_output_value: u64,
    // Total value of non-coinbase inputs (satoshis)
    pub total_input_value: u64,
    // Fee rate percentiles (sat/vB)
    pub fee_rate_p10: f64,
    pub fee_rate_p90: f64,
    // Stamps protocol (bare multisig encoding with specific pattern)
    pub stamps_count: u64,
    // Largest transaction in the block (bytes)
    pub largest_tx_size: u64,
}

impl BitcoinRpc {
    /// Create a new RPC client with 30s request timeout and 10s connect timeout.
    pub fn new(url: String, user: String, password: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .connect_timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("Failed to build HTTP client"),
            url,
            user,
            password,
        }
    }

    /// Send a JSON-RPC 1.0 request to Bitcoin Core.
    async fn call(
        &self,
        method: &str,
        params: &[Value],
    ) -> Result<Value, StatsError> {
        let body = json!({
            "jsonrpc": "1.0",
            "id": "bitcoin_stats",
            "method": method,
            "params": params,
        });

        let resp = self
            .client
            .post(&self.url)
            .basic_auth(&self.user, Some(&self.password))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(StatsError::Rpc(format!(
                "RPC returned status {}",
                resp.status()
            )));
        }

        let mut result: Value = resp.json().await?;

        if let Some(error) = result.get("error") {
            if !error.is_null() {
                return Err(StatsError::Rpc(format!("RPC error: {error}")));
            }
        }

        Ok(result["result"].take())
    }

    /// Call `getblockchaininfo` - returns chain state, tip height, difficulty.
    pub async fn get_blockchain_info(
        &self,
    ) -> Result<BlockchainInfo, StatsError> {
        let result = self.call("getblockchaininfo", &[]).await?;
        serde_json::from_value(result)
            .map_err(|e| StatsError::Rpc(e.to_string()))
    }

    /// Estimated network hash rate (hashes per second).
    pub async fn get_network_hashps(&self) -> Result<f64, StatsError> {
        let result = self.call("getnetworkhashps", &[]).await?;
        result.as_f64().ok_or_else(|| {
            StatsError::Rpc("Expected number for hashps".to_string())
        })
    }

    /// Estimate fee rate (sat/vB) to confirm within `target` blocks.
    pub async fn estimate_smart_fee(
        &self,
        target: u64,
    ) -> Result<f64, StatsError> {
        let result = self.call("estimatesmartfee", &[json!(target)]).await?;
        // Returns feerate in BTC/kvB, convert to sat/vB: * 1e8 / 1000
        let btc_per_kvb = result["feerate"].as_f64().unwrap_or(0.0);
        Ok(btc_per_kvb * 100_000.0) // BTC/kvB -> sat/vB
    }

    /// Call `getblockhash` - returns the block hash at a given height.
    pub async fn get_block_hash(
        &self,
        height: u64,
    ) -> Result<String, StatsError> {
        let result = self.call("getblockhash", &[json!(height)]).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| {
            StatsError::Rpc("Expected string for block hash".to_string())
        })
    }

    /// Fetch a block at verbosity=2 and extract all metrics.
    /// This is the core data extraction function -- parses the full JSON response
    /// to compute fees, median stats, OP_RETURN classification, and miner ID.
    pub async fn get_block(&self, hash: &str) -> Result<Block, StatsError> {
        let result = self.call("getblock", &[json!(hash), json!(2)]).await?;

        let hash = result["hash"].as_str().unwrap_or_default().to_string();
        let height = result["height"].as_u64().unwrap_or(0);
        let time = result["time"].as_u64().unwrap_or(0);
        let n_tx = result["nTx"].as_u64().unwrap_or(0);
        let size = result["size"].as_u64().unwrap_or(0);
        let weight = result["weight"].as_u64().unwrap_or(0);
        let difficulty = result["difficulty"].as_f64().unwrap_or(0.0);
        let version = result["version"].as_u64().unwrap_or(0) as u32;

        // === Coinbase extraction: fees, miner ID, locktime ===
        let mut total_fees = 0u64;
        let mut coinbase_locktime = 0u64;
        let mut coinbase_sequence = 0xFFFF_FFFFu64;
        let mut miner = String::from("Unknown");
        if let Some(txs) = result["tx"].as_array() {
            if let Some(coinbase_tx) = txs.first() {
                // BIP-54 signaling: coinbase nLockTime == height - 1 AND nSequence != 0xffffffff
                coinbase_locktime =
                    coinbase_tx["locktime"].as_u64().unwrap_or(0);
                if let Some(vin) = coinbase_tx["vin"].as_array() {
                    if let Some(first_vin) = vin.first() {
                        coinbase_sequence = first_vin["sequence"]
                            .as_u64()
                            .unwrap_or(0xFFFF_FFFF);
                    }
                }

                // Total fees = coinbase output value - block subsidy
                let mut coinbase_value = 0u64;
                if let Some(vouts) = coinbase_tx["vout"].as_array() {
                    for vout in vouts {
                        if let Some(val) = vout["value"].as_f64() {
                            coinbase_value +=
                                (val * 100_000_000.0).round() as u64;
                        }
                    }
                }
                let subsidy = classifier::block_subsidy(height);
                total_fees = coinbase_value.saturating_sub(subsidy);

                // Miner ID: check coinbase scriptSig first
                if let Some(vin) = coinbase_tx["vin"].as_array() {
                    if let Some(first_vin) = vin.first() {
                        if let Some(coinbase_hex) =
                            first_vin["coinbase"].as_str()
                        {
                            miner = classifier::identify_miner(coinbase_hex);
                        }
                    }
                }

                // Fallback: check coinbase OP_RETURN outputs for miner tag
                if miner == "Unknown" {
                    if let Some(vouts) = coinbase_tx["vout"].as_array() {
                        for vout in vouts {
                            if vout["scriptPubKey"]["type"].as_str()
                                == Some("nulldata")
                            {
                                if let Some(hex) =
                                    vout["scriptPubKey"]["hex"].as_str()
                                {
                                    let found = classifier::identify_miner(hex);
                                    if found != "Unknown" {
                                        miner = found.to_string();
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // === Single pass over non-coinbase txs: fees, OP_RETURN, outputs, inputs, RBF, witness ===
        let mut inscription_count = 0u64;
        let mut inscription_bytes = 0u64;
        let mut brc20_count = 0u64;

        let cap = n_tx.saturating_sub(1) as usize;
        let mut tx_fees: Vec<u64> = Vec::with_capacity(cap);
        let mut tx_fee_rates: Vec<f64> = Vec::with_capacity(cap);
        let mut op_return_count = 0u64;
        let mut op_return_bytes = 0u64;
        let mut runes_count = 0u64;
        let mut runes_bytes = 0u64;
        let mut omni_count = 0u64;
        let mut omni_bytes = 0u64;
        let mut counterparty_count = 0u64;
        let mut counterparty_bytes = 0u64;
        let mut data_carrier_count = 0u64;
        let mut data_carrier_bytes = 0u64;
        let mut segwit_spend_count = 0u64;
        let mut taproot_spend_count = 0u64;
        let mut p2pk_count = 0u64;
        let mut p2pkh_count = 0u64;
        let mut p2sh_count = 0u64;
        let mut p2wpkh_count = 0u64;
        let mut p2wsh_count = 0u64;
        let mut p2tr_count = 0u64;
        let mut multisig_count = 0u64;
        let mut unknown_script_count = 0u64;
        let mut input_count = 0u64;
        let mut output_count = 0u64;
        let mut rbf_count = 0u64;
        let mut witness_bytes = 0u64;
        let mut taproot_keypath_count = 0u64;
        let mut taproot_scriptpath_count = 0u64;
        let mut total_output_value = 0u64;
        let mut total_input_value = 0u64;
        let mut stamps_count = 0u64;
        let mut largest_tx_size = 0u64;

        if let Some(txs) = result["tx"].as_array() {
            for tx in txs.iter().skip(1) {
                // --- Fees ---
                if let Some(fee_btc) = tx["fee"].as_f64() {
                    let fee_sats = (fee_btc * 100_000_000.0).round() as u64;
                    tx_fees.push(fee_sats);
                    if let Some(vsize) = tx["vsize"].as_u64() {
                        if vsize > 0 {
                            tx_fee_rates.push(fee_sats as f64 / vsize as f64);
                        }
                    }
                }

                // --- Inputs: counting, SegWit detection, RBF, witness bytes ---
                // Track largest tx
                if let Some(sz) = tx["size"].as_u64() {
                    if sz > largest_tx_size {
                        largest_tx_size = sz;
                    }
                }

                let mut has_witness = false;
                let mut is_rbf = false;
                if let Some(vins) = tx["vin"].as_array() {
                    input_count += vins.len() as u64;
                    for vin in vins {
                        // Sum input values from prevout
                        if let Some(val) = vin
                            .get("prevout")
                            .and_then(|p| p.get("value"))
                            .and_then(|v| v.as_f64())
                        {
                            total_input_value +=
                                (val * 100_000_000.0).round() as u64;
                        }
                        // Witness detection + byte counting + inscription detection
                        if let Some(wit) = vin["txinwitness"].as_array() {
                            has_witness = true;
                            // Witness overhead: item count varint per input
                            let wit_items = wit.len() as u64;
                            witness_bytes += varint_size(wit_items);
                            for item in wit {
                                if let Some(hex) = item.as_str() {
                                    let item_bytes = (hex.len() as u64) / 2;
                                    // Each item is prefixed by a length varint
                                    witness_bytes += varint_size(item_bytes);
                                    witness_bytes += item_bytes;
                                    // Ordinals inscription envelope:
                                    // OP_FALSE(00) OP_IF(63) OP_PUSH3(03) "ord"(6f7264)
                                    if hex.contains("0063036f7264") {
                                        inscription_count += 1;
                                        // Calculate actual envelope overhead instead of fixed estimate.
                                        // Envelope: OP_FALSE(1) OP_IF(1) OP_PUSH3(1) "ord"(3) = 6 bytes header
                                        // + content-type marker OP_PUSH1(1) + type length(1) + type bytes(variable)
                                        // + separator OP_0(1) + content-length push(1-3) + OP_ENDIF(1)
                                        // Approximate: find envelope start, subtract from total
                                        let overhead = if let Some(env_pos) =
                                            hex.find("0063036f7264")
                                        {
                                            // Bytes before envelope + envelope header (6 bytes = 12 hex chars)
                                            // + content-type section (scan for 00 separator after header)
                                            let after_header = env_pos + 12; // past "0063036f7264"
                                                                             // Find OP_0 separator (00) after content-type
                                            let separator_pos = hex
                                                [after_header..]
                                                .find("00")
                                                .map(|p| after_header + p)
                                                .unwrap_or(after_header);
                                            // overhead = header(6) + content-type section + separator(1) + OP_ENDIF(1)
                                            let ct_bytes = (separator_pos
                                                - after_header)
                                                / 2;
                                            (6 + ct_bytes + 2) as u64
                                        } else {
                                            10 // fallback
                                        };
                                        inscription_bytes +=
                                            item_bytes.saturating_sub(overhead);
                                        // BRC-20: inscription containing {"p":"brc-20"
                                        if hex.contains(
                                            "7b2270223a226272632d3230",
                                        ) {
                                            brc20_count += 1;
                                        }
                                    }
                                }
                            }
                            // Taproot spend type detection:
                            // Key-path: exactly 1 witness element (64-65 byte Schnorr sig)
                            // Script-path: 2+ elements, last element starts with 0xc0 or 0xc1
                            //              (taproot control block version byte)
                            let wit_len = wit.len();
                            if wit_len == 1 {
                                // Likely key-path: single element should be 64-65 bytes (128-130 hex chars)
                                if let Some(hex) = wit[0].as_str() {
                                    let byte_len = hex.len() / 2;
                                    if byte_len == 64 || byte_len == 65 {
                                        taproot_keypath_count += 1;
                                    }
                                }
                            } else if wit_len >= 2 {
                                // Check if last element is a taproot control block (starts with c0 or c1)
                                if let Some(last) =
                                    wit.last().and_then(|v| v.as_str())
                                {
                                    if last.starts_with("c0")
                                        || last.starts_with("c1")
                                    {
                                        taproot_scriptpath_count += 1;
                                    }
                                }
                            }
                        }
                        // RBF: nSequence < 0xFFFFFFFE signals replaceability,
                        // BUT exclude inputs spending CSV timelocks (BIP 68) —
                        // those use low nSequence for relative lock-time, not RBF.
                        if let Some(seq) = vin["sequence"].as_u64() {
                            if seq < 0xFFFF_FFFE {
                                let is_csv = vin
                                    .get("prevout")
                                    .and_then(|p| p.get("scriptPubKey"))
                                    .and_then(|s| s.get("asm"))
                                    .and_then(|a| a.as_str())
                                    .is_some_and(|asm| {
                                        asm.contains("OP_CHECKSEQUENCEVERIFY")
                                    });
                                if !is_csv {
                                    is_rbf = true;
                                }
                            }
                        }
                    }
                }
                if has_witness {
                    segwit_spend_count += 1;
                    // BIP 141 witness marker (0x00) + flag (0x01) = 2 bytes per witness tx
                    witness_bytes += 2;
                }
                if is_rbf {
                    rbf_count += 1;
                }

                // --- Outputs: counting, script type classification, OP_RETURN ---
                if let Some(vouts) = tx["vout"].as_array() {
                    output_count += vouts.len() as u64;
                    for vout in vouts {
                        // Sum output values (excludes coinbase — measures actual transfers)
                        if let Some(val) = vout["value"].as_f64() {
                            total_output_value +=
                                (val * 100_000_000.0).round() as u64;
                        }
                        match vout["scriptPubKey"]["type"].as_str() {
                            Some("pubkey") => p2pk_count += 1,
                            Some("pubkeyhash") => p2pkh_count += 1,
                            Some("scripthash") => p2sh_count += 1,
                            Some("witness_v0_keyhash") => p2wpkh_count += 1,
                            Some("witness_v0_scripthash") => p2wsh_count += 1,
                            Some("witness_v1_taproot") => {
                                p2tr_count += 1;
                                taproot_spend_count += 1;
                            }
                            Some("nulldata") => {
                                // OP_RETURN classification
                                if let Some(hex) =
                                    vout["scriptPubKey"]["hex"].as_str()
                                {
                                    let bytes = (hex.len() as u64) / 2;
                                    let classification =
                                        classifier::classify(hex, height);
                                    match classification {
                                        OpReturnType::SegwitCommit => continue,
                                        OpReturnType::Runes => {
                                            runes_count += 1;
                                            runes_bytes += bytes;
                                        }
                                        OpReturnType::Omni => {
                                            omni_count += 1;
                                            omni_bytes += bytes;
                                        }
                                        OpReturnType::Counterparty => {
                                            counterparty_count += 1;
                                            counterparty_bytes += bytes;
                                        }
                                        OpReturnType::DataCarrier => {
                                            data_carrier_count += 1;
                                            data_carrier_bytes += bytes;
                                        }
                                    }
                                    op_return_count += 1;
                                    op_return_bytes += bytes;
                                }
                            }
                            Some("multisig") => {
                                multisig_count += 1;
                                // Stamps: bare multisig with fake pubkeys (data encoding).
                                // Real compressed pubkeys start with 02 or 03. If the hex
                                // contains keys that don't, it's likely a Stamps output.
                                if let Some(hex) =
                                    vout["scriptPubKey"]["hex"].as_str()
                                {
                                    // Stamps multisig pattern: 1-of-N where N keys contain data.
                                    // The asm starts with "1 <key1> <key2> ... N OP_CHECKMULTISIG"
                                    // Check if any 33-byte "key" doesn't start with 02/03.
                                    if let Some(asm) =
                                        vout["scriptPubKey"]["asm"].as_str()
                                    {
                                        let parts: Vec<&str> =
                                            asm.split_whitespace().collect();
                                        // Valid multisig: M key1 key2 ... N OP_CHECKMULTISIG
                                        if parts.len() >= 4 {
                                            let has_fake_key = parts
                                                [1..parts.len() - 2]
                                                .iter()
                                                .any(|k| {
                                                    k.len() == 66
                                                        && !k.starts_with("02")
                                                        && !k.starts_with("03")
                                                });
                                            if has_fake_key {
                                                stamps_count += 1;
                                            }
                                        }
                                    }
                                }
                            }
                            Some("nonstandard") => {
                                unknown_script_count += 1;
                            }
                            _ => unknown_script_count += 1,
                        }
                    }
                }
            }
        }

        tx_fees.sort_unstable();
        tx_fee_rates.sort_unstable_by(|a, b| {
            a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
        });
        let median_fee = if tx_fees.is_empty() {
            0
        } else {
            tx_fees[(tx_fees.len() - 1) / 2]
        };
        let median_fee_rate = if tx_fee_rates.is_empty() {
            0.0
        } else {
            tx_fee_rates[(tx_fee_rates.len() - 1) / 2]
        };
        let fee_rate_p10 = if tx_fee_rates.len() >= 10 {
            tx_fee_rates[tx_fee_rates.len() / 10]
        } else {
            0.0
        };
        let fee_rate_p90 = if tx_fee_rates.len() >= 10 {
            tx_fee_rates[tx_fee_rates.len() * 9 / 10]
        } else {
            0.0
        };

        Ok(Block {
            hash,
            height,
            time,
            n_tx,
            size,
            weight,
            difficulty,
            op_return_count,
            op_return_bytes,
            runes_count,
            runes_bytes,
            omni_count,
            omni_bytes,
            counterparty_count,
            counterparty_bytes,
            data_carrier_count,
            data_carrier_bytes,
            version,
            total_fees,
            median_fee,
            median_fee_rate,
            coinbase_locktime,
            coinbase_sequence,
            miner,
            segwit_spend_count,
            taproot_spend_count,
            taproot_keypath_count,
            taproot_scriptpath_count,
            p2pk_count,
            p2pkh_count,
            p2sh_count,
            p2wpkh_count,
            p2wsh_count,
            p2tr_count,
            multisig_count,
            unknown_script_count,
            input_count,
            output_count,
            rbf_count,
            witness_bytes,
            inscription_count,
            inscription_bytes,
            brc20_count,
            total_output_value,
            total_input_value,
            fee_rate_p10,
            fee_rate_p90,
            stamps_count,
            largest_tx_size,
        })
    }

    /// Convenience method: fetch a block by height (calls getblockhash then getblock).
    pub async fn fetch_block_by_height(
        &self,
        height: u64,
    ) -> Result<Block, StatsError> {
        let hash = self.get_block_hash(height).await?;
        self.get_block(&hash).await
    }

    /// Get mempool entry for a specific txid. Returns fee (sats) and vsize.
    pub async fn get_mempool_entry(
        &self,
        txid: &str,
    ) -> Result<MempoolEntryInfo, StatsError> {
        let result = self.call("getmempoolentry", &[json!(txid)]).await?;
        let fee_btc = result["fees"]["base"]
            .as_f64()
            .or_else(|| result["fee"].as_f64())
            .ok_or_else(|| {
                StatsError::Rpc(format!(
                    "No fee field in mempool entry for {txid}"
                ))
            })?;
        let fee_sats = (fee_btc * 100_000_000.0).round() as u64;
        let vsize = result["vsize"].as_u64().unwrap_or(0) as u32;
        Ok(MempoolEntryInfo {
            fee: fee_sats,
            vsize,
        })
    }

    /// Get block header (fast, ~1KB response). Used for immediate block broadcast.
    pub async fn get_block_header(
        &self,
        hash: &str,
    ) -> Result<BlockTxids, StatsError> {
        let result = self.call("getblockheader", &[json!(hash)]).await?;
        Ok(BlockTxids {
            height: result["height"].as_u64().unwrap_or(0),
            timestamp: result["time"].as_u64().unwrap_or(0),
            size: 0,     // not in header
            weight: 0,   // not in header
            tx_count: result["nTx"].as_u64().unwrap_or(0),
            txids: vec![], // not in header
        })
    }

    /// Get block metadata + txid list (verbosity=1, no full tx data).
    pub async fn get_block_txids(
        &self,
        hash: &str,
    ) -> Result<BlockTxids, StatsError> {
        let result = self.call("getblock", &[json!(hash), json!(1)]).await?;
        let height = result["height"].as_u64().unwrap_or(0);
        let timestamp = result["time"].as_u64().unwrap_or(0);
        let size = result["size"].as_u64().unwrap_or(0);
        let weight = result["weight"].as_u64().unwrap_or(0);
        let tx_count = result["nTx"].as_u64().unwrap_or(0);
        let txids = result["tx"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        Ok(BlockTxids {
            height,
            timestamp,
            size,
            weight,
            tx_count,
            txids,
        })
    }

    /// Get all mempool entries with fee/vsize data (verbose=true).
    /// Used on startup to seed the mempool_txs table for full coverage.
    pub async fn get_raw_mempool_verbose(
        &self,
    ) -> Result<Vec<(String, u64, u32)>, StatsError> {
        let result = self.call("getrawmempool", &[json!(true)]).await?;
        let obj = result
            .as_object()
            .ok_or_else(|| StatsError::Rpc("expected object".to_string()))?;
        let mut entries = Vec::with_capacity(obj.len());
        for (txid, info) in obj {
            let fee_btc = info["fees"]["base"].as_f64().unwrap_or(0.0);
            let fee_sats = (fee_btc * 100_000_000.0).round() as u64;
            let vsize = info["vsize"].as_u64().unwrap_or(0) as u32;
            if vsize > 0 {
                entries.push((txid.clone(), fee_sats, vsize));
            }
        }
        Ok(entries)
    }

    /// Get total fee for a block via getblockstats (returns sats).
    /// Used as fallback when mempool_txs table is sparsely populated.
    pub async fn get_block_total_fee(
        &self,
        height: u64,
    ) -> Result<u64, StatsError> {
        let result = self
            .call(
                "getblockstats",
                &[json!(height), json!(["totalfee"])],
            )
            .await?;
        Ok(result["totalfee"].as_u64().unwrap_or(0))
    }

    /// Call `getmempoolinfo` - returns mempool size, fee stats, memory usage.
    pub async fn get_mempool_info(&self) -> Result<MempoolInfo, StatsError> {
        let result = self.call("getmempoolinfo", &[]).await?;
        serde_json::from_value(result)
            .map_err(|e| StatsError::Rpc(e.to_string()))
    }

    /// Fetch UTXO set info with hash_type="none" (faster, skips hash computation).
    pub async fn get_txout_set_info(&self) -> Result<TxoutSetInfo, StatsError> {
        // gettxoutsetinfo scans the entire UTXO set (~165M entries).
        // Takes 60-120s on fast hardware, up to 5-10 min on a Raspberry Pi.
        let long_client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| StatsError::Rpc(e.to_string()))?;
        let body = serde_json::json!({
            "jsonrpc": "1.0",
            "id": "utxo",
            "method": "gettxoutsetinfo",
            "params": [json!("none")]
        });
        let resp = long_client
            .post(&self.url)
            .basic_auth(&self.user, Some(&self.password))
            .json(&body)
            .send()
            .await
            .map_err(|e| StatsError::Rpc(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(StatsError::Rpc(format!(
                "RPC returned status {}",
                resp.status()
            )));
        }
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| StatsError::Rpc(e.to_string()))?;
        serde_json::from_value(json["result"].clone())
            .map_err(|e| StatsError::Rpc(e.to_string()))
    }

    /// Fetch BTC/USD price from mempool.space API.
    pub async fn fetch_price(&self) -> Result<PriceInfo, StatsError> {
        let resp = self
            .client
            .get("https://mempool.space/api/v1/prices")
            .send()
            .await?;
        let price: PriceInfo = resp.json().await?;
        Ok(price)
    }

    /// Fetch full historical daily BTC/USD prices from blockchain.info.
    /// Returns Vec of (timestamp_ms, price_usd) covering all available history.
    pub async fn fetch_price_history_all(
        &self,
    ) -> Result<Vec<(u64, f64)>, StatsError> {
        let url =
            "https://api.blockchain.info/charts/market-price?timespan=all&format=json";
        let resp = self.client.get(url).send().await?;

        if !resp.status().is_success() {
            return Err(StatsError::Rpc(format!(
                "blockchain.info returned {}",
                resp.status()
            )));
        }

        let body: serde_json::Value = resp.json().await?;
        let values = body["values"].as_array().ok_or_else(|| {
            StatsError::Rpc(
                "No values array in blockchain.info response".into(),
            )
        })?;

        let result: Vec<(u64, f64)> = values
            .iter()
            .filter_map(|p| {
                let obj = p.as_object()?;
                let ts = obj.get("x")?.as_u64()?;
                let price = obj.get("y")?.as_f64()?;
                Some((ts * 1000, price))
            })
            .collect();

        Ok(result)
    }
}
