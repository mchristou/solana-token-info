use reqwest;
use serde_json::Value;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::fmt;

use crate::error::{Error, Result};

const RPC_URL: &str = "https://api.mainnet-beta.solana.com";

lazy_static::lazy_static! {
    static ref RPC_CLIENT: RpcClient =RpcClient::new(RPC_URL.to_string());
}
#[derive(Debug)]
pub struct TokenInfo {
    pub account_info: TokenAccountInfo,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug)]
pub struct TokenAccountInfo {
    pub total_supply: String,
}

impl TokenInfo {
    pub async fn new(pubkey: Pubkey) -> Result<TokenInfo> {
        let (account_info, metadata) =
            tokio::try_join!(TokenAccountInfo::new(pubkey), TokenMetadata::new(pubkey))?;

        let mut metadata_map = HashMap::with_capacity(10);
        if !metadata.uri.is_empty() {
            let additional_info =
                TokenMetadata::fetch_additional_information(&metadata.uri).await?;
            metadata_map.extend(additional_info);
        }

        metadata_map.insert("name".to_string(), metadata.name);
        metadata_map.insert("symbol".to_string(), metadata.symbol);
        metadata_map.insert("uri".to_string(), metadata.uri);

        Ok(TokenInfo {
            account_info,
            metadata: metadata_map,
        })
    }
}

impl TokenAccountInfo {
    pub async fn new(pubkey: Pubkey) -> Result<TokenAccountInfo> {
        let ui_token_amount = RPC_CLIENT.get_token_supply(&pubkey).await?;
        let total_supply = Self::format_amount(ui_token_amount.amount, ui_token_amount.decimals)?;
        Ok(TokenAccountInfo { total_supply })
    }

    fn format_amount(amount: String, decimals: u8) -> Result<String> {
        let amount: u64 = amount.parse()?;
        let divisor = 10u64.pow(decimals as u32);
        let whole_part = amount / divisor;
        let fractional_part = amount % divisor;

        let formatted_amount = if decimals > 0 {
            format!(
                "{}.{:0width$}",
                whole_part,
                fractional_part,
                width = decimals as usize
            )
        } else {
            whole_part.to_string()
        };

        Ok(formatted_amount
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string())
    }
}

#[derive(Debug)]
pub struct TokenMetadata {
    pub name: String,
    pub symbol: String,
    pub uri: String,
}

impl TokenMetadata {
    pub async fn new(pubkey: Pubkey) -> Result<TokenMetadata> {
        let (pubkey_metadata, _) = mpl_token_metadata::accounts::Metadata::find_pda(&pubkey);
        let account = RPC_CLIENT.get_account(&pubkey_metadata).await?;
        let data = account.data;
        let metadata = mpl_token_metadata::accounts::Metadata::from_bytes(&data)?;

        Ok(TokenMetadata {
            name: metadata.name,
            symbol: metadata.symbol,
            uri: metadata.uri,
        })
    }

    async fn fetch_additional_information(uri: &str) -> Result<HashMap<String, String>> {
        let mut additional_info = HashMap::new();

        if uri.is_empty() {
            return Ok(additional_info);
        }

        let response = reqwest::get(uri)
            .await
            .map_err(|_| Error::Generic("Failed to get additional information".to_string()))?;
        if response.status().is_success() {
            let json_data: Value = response
                .json()
                .await
                .map_err(|_| Error::Generic("Failed to parse JSON".to_string()))?;
            if let Some(object) = json_data.as_object() {
                additional_info.extend(
                    object
                        .iter()
                        .map(|(key, value)| (key.clone(), value.to_string())),
                );
            }
        }

        if let Some(website) = additional_info.get("website") {
            let trimmed_website = website.trim_matches(|c| c == '"' || c == ' ');
            let trimmed_website = trimmed_website.trim_start_matches("https://");
            let resolver = trust_dns_resolver::AsyncResolver::tokio(
                trust_dns_resolver::config::ResolverConfig::default(),
                trust_dns_resolver::config::ResolverOpts::default(),
            );
            let response = resolver.lookup_ip(trimmed_website).await.unwrap();
            let count = response.iter().count();

            additional_info.insert(
                "number of website DNS records".to_string(),
                count.to_string(),
            );
        }

        Ok(additional_info)
    }
}

impl fmt::Display for TokenInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\n{}\n{}\n",
            self.account_info,
            self.metadata
                .iter()
                .map(|(key, value)| format!("{}: {}", key, value))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

impl fmt::Display for TokenAccountInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "total supply: {}", self.total_supply)
    }
}
