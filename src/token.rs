use mpl_token_metadata::accounts::Metadata;
use serde_json::Value;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::{collections::HashMap, fmt, ops::Deref};
use tokio::sync::OnceCell;
use tracing::{debug, error, instrument, trace};

use crate::error::{Error, Result};

static RPC_CLIENT: OnceCell<RpcClient> = OnceCell::const_new();
const RPC_URL: &str = "https://api.mainnet-beta.solana.com";

async fn rpc_client() -> &'static RpcClient {
    RPC_CLIENT
        .get_or_init(|| async { RpcClient::new(RPC_URL.to_string()) })
        .await
}

/// Represents information about a token, including account info, metadata, and additional information.
#[derive(Debug)]
pub struct TokenInfo {
    /// The account information of the token.
    pub account_info: TokenAccountInfo,
    /// Additional information fetched from the metadata URI.
    pub additional_info: HashMap<String, String>,
    /// Metadata of the token.
    pub metadata: Option<TokenMetadata>,
}

/// Represents account information for a token, including the total supply.
#[derive(Debug)]
pub struct TokenAccountInfo {
    /// The public key of the owner.
    pub owner: Pubkey,
    /// The total supply of the token as a formatted string.
    pub total_supply: String,
}

impl TokenInfo {
    /// Creates a new `TokenInfo` instance for the given public key.
    ///
    /// This function fetches account info, metadata, and additional information.
    ///
    /// # Arguments
    ///
    /// * `pubkey` - The public key of the token account.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `TokenInfo` or an error.
    #[instrument]
    pub async fn new(pubkey: Pubkey) -> Result<Self> {
        debug!("Collecting information for: {pubkey}");

        let (account_info_res, metadata_res) =
            tokio::join!(TokenAccountInfo::new(pubkey), TokenMetadata::new(pubkey));

        let account_info = account_info_res?;

        let (metadata, additional_info) = match metadata_res {
            Ok(metadata) => {
                let additional_info_res =
                    TokenMetadata::fetch_additional_information(&metadata.uri).await;
                let additional_info = match additional_info_res {
                    Ok(info) => info,
                    Err(_) if metadata.uri.is_empty() => {
                        trace!("Token does not have URI to fetch additional information");
                        HashMap::new()
                    }
                    Err(e) => {
                        trace!("Failed to fetch metadata, error: {e}");
                        HashMap::new()
                    }
                };
                (Some(metadata), additional_info)
            }
            Err(e) => {
                error!("{e}");
                (None, HashMap::new())
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
    /// Creates a new `TokenAccountInfo` instance for the given public key.
    ///
    /// This function fetches the token supply and formats it.
    ///
    /// # Arguments
    ///
    /// * `pubkey` - The public key of the token account.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `TokenAccountInfo` or an error.
    pub async fn new(pubkey: Pubkey) -> Result<Self> {
        let (ui_token_amount, account_info) = tokio::join!(
            rpc_client().await.get_token_supply(&pubkey),
            rpc_client().await.get_account(&pubkey)
        );

        let account_info = account_info?;
        let ui_token_amount = ui_token_amount?;

        let total_supply = Self::format_amount(ui_token_amount.amount, ui_token_amount.decimals)?;
        Ok(Self {
            owner: account_info.owner,
            total_supply,
        })
    }

    /// Formats the token amount by applying the correct number of decimals.
    ///
    /// # Arguments
    ///
    /// * `amount` - The raw amount as a string.
    /// * `decimals` - The number of decimal places.
    ///
    /// # Returns
    ///
    /// A `Result` containing the formatted amount as a string, or an error.
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

/// Represents the metadata of a token, wrapping around the `mpl_token_metadata::accounts::Metadata`.
#[derive(Debug)]
pub struct TokenMetadata {
    /// The metadata account of the token.
    metadata: Metadata,
}

impl TokenMetadata {
    /// Creates a new `TokenMetadata` instance for the given public key.
    ///
    /// This function fetches the metadata from the Solana blockchain.
    ///
    /// # Arguments
    ///
    /// * `pubkey` - The public key of the token account.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `TokenMetadata` or an error.
    pub async fn new(pubkey: Pubkey) -> Result<Self> {
        let (pubkey_metadata, _) = mpl_token_metadata::accounts::Metadata::find_pda(&pubkey);
        let account = rpc_client().await.get_account(&pubkey_metadata).await?;

        let metadata = Metadata::from_bytes(&account.data)?;

        Ok(Self { metadata })
    }

    /// Fetches additional information from the metadata URI.
    ///
    /// This function sends an HTTP GET request to the URI, parses the JSON response,
    /// and extracts additional information into a `HashMap`.
    ///
    /// # Arguments
    ///
    /// * `uri` - The URI to fetch additional information from.
    ///
    /// # Returns
    ///
    /// A `Result` containing the additional information as a `HashMap`, or an error.
    async fn fetch_additional_information(uri: &str) -> Result<HashMap<String, String>> {
        let mut additional_info = HashMap::new();

        if uri.is_empty() || uri.bytes().all(|b| b == 0) {
            trace!("Metadata do not include uri, website will not be retrieved");
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
        if let Some(metadata) = self.metadata.as_ref() {
            write!(
                f,
                "\n{}\n\n{}\ninformation retrieved using uri:\n{}",
                self.account_info,
                metadata,
                self.additional_info
                    .iter()
                    .map(|(key, value)| format!("{}: {}", key.to_lowercase(), value.to_lowercase()))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        } else {
            write!(f, "\n{}", self.account_info,)
        }
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
