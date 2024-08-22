use mpl_token_metadata::accounts::Metadata;
use serde_json::Value;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::{collections::HashMap, fmt, ops::Deref};
use tokio::sync::OnceCell;
use tracing::warn;

use crate::error::{Error, Result};

const RPC_URL: &str = "https://api.mainnet-beta.solana.com";

static RPC_CLIENT: OnceCell<RpcClient> = OnceCell::const_new();

#[derive(Debug)]
pub struct TokenInfo {
    pub account_info: TokenAccountInfo,
    pub additional_info: HashMap<String, String>,
    pub metadata: TokenMetadata,
}

#[derive(Debug)]
pub struct TokenAccountInfo {
    pub total_supply: String,
}

impl TokenInfo {
    pub async fn new(pubkey: Pubkey) -> Result<Self> {
        let (account_info, metadata) =
            tokio::try_join!(TokenAccountInfo::new(pubkey), TokenMetadata::new(pubkey))?;

        let additional_info = match TokenMetadata::fetch_additional_information(&metadata.uri).await
        {
            Ok(info) => info,
            Err(_) if metadata.uri.is_empty() => {
                warn!("Token does not have URI to fetch additional information");
                HashMap::new()
            }
            Err(e) => {
                warn!("{e}");
                HashMap::new()
            }
        };

        Ok(Self {
            account_info,
            additional_info,
            metadata,
        })
    }
}

impl TokenAccountInfo {
    pub async fn new(pubkey: Pubkey) -> Result<Self> {
        let ui_token_amount = RPC_CLIENT
            .get_or_init(|| async { RpcClient::new(RPC_URL.to_string()) })
            .await
            .get_token_supply(&pubkey)
            .await?;

        let total_supply = Self::format_amount(ui_token_amount.amount, ui_token_amount.decimals)?;
        Ok(Self { total_supply })
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
    metadata: Metadata,
}

impl TokenMetadata {
    pub async fn new(pubkey: Pubkey) -> Result<Self> {
        let (pubkey_metadata, _) = mpl_token_metadata::accounts::Metadata::find_pda(&pubkey);
        let account = RPC_CLIENT
            .get_or_init(|| async { RpcClient::new(RPC_URL.to_string()) })
            .await
            .get_account(&pubkey_metadata)
            .await?;

        let metadata = Metadata::from_bytes(&account.data)?;

        Ok(Self { metadata })
    }

    async fn fetch_additional_information(uri: &str) -> Result<HashMap<String, String>> {
        let mut additional_info = HashMap::new();

        if uri.is_empty() || uri.bytes().all(|b| b == 0) {
            warn!("Metadata do not include uri, website will not be retrieved");
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

        // Check the website to count DNS records.
        if let Some(website) = additional_info.get("website") {
            let trimmed_website = website
                .trim_matches(|c| c == '"' || c == ' ')
                .trim_start_matches("https://");
            let resolver = trust_dns_resolver::AsyncResolver::tokio(
                trust_dns_resolver::config::ResolverConfig::default(),
                trust_dns_resolver::config::ResolverOpts::default(),
            );
            let response = resolver.lookup_ip(trimmed_website).await.unwrap();
            let count = response.iter().count();

            additional_info.insert(
                "number of website dns records".to_string(),
                count.to_string(),
            );
        }

        Ok(additional_info)
    }
}

impl Deref for TokenMetadata {
    type Target = Metadata;

    fn deref(&self) -> &Self::Target {
        &self.metadata
    }
}

impl fmt::Display for TokenAccountInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "total supply: {}", self.total_supply)
    }
}

impl fmt::Display for TokenInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}\n\n{}{}",
            self.account_info,
            self.metadata,
            self.additional_info
                .iter()
                .map(|(key, value)| format!("{}: {}", key.to_lowercase(), value.to_lowercase()))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

impl fmt::Display for TokenMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "information collected from metadata:\n\
            key: {:?}\n\
            update authority: {}\n\
            mint: {}\n\
            name: {}\n\
            symbol: {}\n\
            uri: {}\n\
            seller fee basis points: {}\n\
            creators: {:?}\n\
            primary sale happened: {}\n\
            is mutable: {}\n\
            edition nonce: {:?}\n\
            token standard: {:?}\n\
            collection: {:?}\n\
            uses: {:?}\n\
            collection details: {:?}\n\
            programmable config: {:?}\n",
            self.metadata.key,
            self.metadata.update_authority,
            self.metadata.mint,
            self.metadata.name,
            self.metadata.symbol,
            self.metadata.uri,
            self.metadata.seller_fee_basis_points,
            self.metadata.creators,
            self.metadata.primary_sale_happened,
            self.metadata.is_mutable,
            self.metadata.edition_nonce,
            self.metadata.token_standard,
            self.metadata.collection,
            self.metadata.uses,
            self.metadata.collection_details,
            self.metadata.programmable_config
        )
    }
}
