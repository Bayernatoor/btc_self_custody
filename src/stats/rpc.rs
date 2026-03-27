//! Bitcoin Core JSON-RPC client.
//!
//! Thin wrapper over reqwest that handles authentication and JSON-RPC protocol.
//! Fetches blocks at verbosity=2 (full transaction data) to extract:
//! - Block metadata (size, weight, difficulty, version)
//! - Fee statistics (total fees, median fee, median fee rate)
//! - OP_RETURN classification (Runes vs data carriers)
//! - Miner identification (from coinbase scriptSig and OP_RETURN outputs)
//! - BIP-54 signaling (coinbase locktime)

use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

use super::classifier::{self, OpReturnType};
use super::error::StatsError;

pub struct BitcoinRpc {
    client: Client,
    url: String,
    user: String,
    password: String,
}

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

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct TxoutSetInfo {
    pub txouts: u64,
    pub total_amount: f64,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct PriceInfo {
    #[serde(rename = "USD")]
    pub usd: f64,
    pub time: u64,
}

/// Parsed block data from getblock verbosity=2.
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
}

impl BitcoinRpc {
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
        result
            .as_f64()
            .ok_or_else(|| StatsError::Rpc("Expected number for hashps".to_string()))
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
        let mut miner: &str = "Unknown";
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
                                        miner = found;
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
                let mut has_witness = false;
                let mut is_rbf = false;
                if let Some(vins) = tx["vin"].as_array() {
                    input_count += vins.len() as u64;
                    for vin in vins {
                        // Witness detection + byte counting + inscription detection
                        if let Some(wit) = vin["txinwitness"].as_array() {
                            has_witness = true;
                            for item in wit {
                                if let Some(hex) = item.as_str() {
                                    let item_bytes = (hex.len() as u64) / 2;
                                    witness_bytes += item_bytes;
                                    // Ordinals inscription envelope:
                                    // OP_FALSE(00) OP_IF(63) OP_PUSH3(03) "ord"(6f7264)
                                    if hex.contains("0063036f7264") {
                                        inscription_count += 1;
                                        inscription_bytes += item_bytes.saturating_sub(10);
                                        // BRC-20: inscription containing {"p":"brc-20"
                                        if hex.contains("7b2270223a226272632d3230") {
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
                                if let Some(last) = wit.last().and_then(|v| v.as_str()) {
                                    if last.starts_with("c0") || last.starts_with("c1") {
                                        taproot_scriptpath_count += 1;
                                    }
                                }
                            }
                        }
                        // RBF: nSequence < 0xFFFFFFFE signals replaceability
                        if let Some(seq) = vin["sequence"].as_u64() {
                            if seq < 0xFFFF_FFFE {
                                is_rbf = true;
                            }
                        }
                    }
                }
                if has_witness {
                    segwit_spend_count += 1;
                }
                if is_rbf {
                    rbf_count += 1;
                }

                // --- Outputs: counting, script type classification, OP_RETURN ---
                if let Some(vouts) = tx["vout"].as_array() {
                    output_count += vouts.len() as u64;
                    for vout in vouts {
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
                                if let Some(hex) = vout["scriptPubKey"]["hex"].as_str() {
                                    let bytes = (hex.len() as u64) / 2;
                                    let classification = classifier::classify(hex);
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
                            Some("multisig") => multisig_count += 1,
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
            miner: miner.to_string(),
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
        })
    }

    pub async fn fetch_block_by_height(
        &self,
        height: u64,
    ) -> Result<Block, StatsError> {
        let hash = self.get_block_hash(height).await?;
        self.get_block(&hash).await
    }

    pub async fn get_mempool_info(&self) -> Result<MempoolInfo, StatsError> {
        let result = self.call("getmempoolinfo", &[]).await?;
        serde_json::from_value(result)
            .map_err(|e| StatsError::Rpc(e.to_string()))
    }

    /// Fetch UTXO set info with hash_type="none" (faster, skips hash computation).
    pub async fn get_txout_set_info(&self) -> Result<TxoutSetInfo, StatsError> {
        // gettxoutsetinfo scans the entire UTXO set — can take 60-120s.
        // Use a dedicated client with longer timeout.
        let long_client = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
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
        let resp = self
            .client
            .get(url)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(StatsError::Rpc(format!(
                "blockchain.info returned {}",
                resp.status()
            )));
        }

        let body: serde_json::Value = resp.json().await?;
        let values = body["values"]
            .as_array()
            .ok_or_else(|| StatsError::Rpc("No values array in blockchain.info response".into()))?;

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
