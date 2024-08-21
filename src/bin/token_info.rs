use argh::FromArgs;
use eyre::Result;
use solana_sdk::pubkey::Pubkey;
use solana_token_info::*;

#[derive(FromArgs, Debug)]
/// Simple CLI that returns token information
struct TokenInfoArgs {
    /// token public key
    #[argh(positional)]
    pubkey: Pubkey,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: TokenInfoArgs = argh::from_env();

    let start = std::time::Instant::now();

    match TokenInfo::new(args.pubkey).await {
        Ok(token_info) => println!(
            "Token Info: {}\nTime taken: {:?}",
            token_info,
            start.elapsed()
        ),
        Err(e) => eprintln!("Error: {:?}", e),
    }

    Ok(())
}
