use argh::FromArgs;
use eyre::Result;
use futures::future::join_all;
use solana_sdk::pubkey::Pubkey;
use solana_token_info::*;
use tokio::{task::JoinHandle, time::Instant};
use tracing::{debug, error, info};

#[derive(FromArgs, Debug)]
/// Simple CLI that returns token information
struct TokenInfoArgs {
    /// token public key
    #[argh(positional)]
    pubkeys: Vec<Pubkey>,
}

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "warn,solana_token_info=warn,token_info=info");
    }
    tracing_subscriber::fmt::init();

    let args: TokenInfoArgs = argh::from_env();
    let now = Instant::now();

    let tokens_info_tasks: Vec<_> = args
        .pubkeys
        .into_iter()
        .map(|pubkey| -> JoinHandle<Result<()>> {
            tokio::task::spawn(async move {
                debug!("Fetching info for: {}", pubkey);
                match TokenInfo::new(pubkey).await {
                    Ok(token_info) => {
                        info!("Information collected for: {pubkey}");
                        info!(
                            "Token Info: {}\n\nTime taken: {:?}\n\n",
                            token_info,
                            now.elapsed()
                        )
                    }
                    Err(e) => error!("Error: {:?}", e),
                };

                Ok(())
            })
        })
        .collect();

    let _ = join_all(tokens_info_tasks).await;
    info!("Total elpased time {:#?}", now.elapsed());

    Ok(())
}
