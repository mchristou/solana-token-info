use argh::FromArgs;
use eyre::Result;
use solana_sdk::pubkey::Pubkey;
use solana_token_info::*;
use tracing::{error, info};

#[derive(FromArgs, Debug)]
/// Simple CLI that returns token information
struct TokenInfoArgs {
    /// token public key
    #[argh(positional)]
    pubkey: Pubkey,
}

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "warn,solana_token_info=warn,token_info=info");
    }
    tracing_subscriber::fmt::init();

    let args: TokenInfoArgs = argh::from_env();

    let start = std::time::Instant::now();

    info!("Fetching info for: {}", args.pubkey);
    match TokenInfo::new(args.pubkey).await {
        Ok(token_info) => info!(
            "Token Info: {}\n\nTime taken: {:?}",
            token_info,
            start.elapsed()
        ),
        Err(e) => error!("Error: {:?}", e),
    }

    Ok(())
}
