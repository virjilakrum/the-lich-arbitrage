use the_lich::{Config, Result};
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    init_logging()?;

    info!("Starting The Lich - Solana Arbitrage Bot v{}", the_lich::VERSION);

    // Load configuration
    let config = match Config::load() {
        Ok(config) => {
            info!("Configuration loaded successfully");
            config
        }
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            return Err(e);
        }
    };

    // Check if bot is enabled
    if !config.bot.enabled {
        info!("Bot is disabled in configuration. Exiting.");
        return Ok(());
    }

    info!("Bot configuration:");
    info!("  Environment: {}", config.bot.environment);
    info!("  Primary RPC endpoints: {}", config.network.primary_rpc.len());
    info!("  Monitored pairs: {}", config.strategy.monitored_pairs.len());
    info!("  Min profit threshold: {}%", config.strategy.min_profit_threshold * rust_decimal::Decimal::from(100));

    // TODO: Initialize and start the bot components
    // This would include:
    // 1. Network manager
    // 2. Data manager
    // 3. Arbitrage engine
    // 4. Transaction manager
    // 5. Monitoring system

    info!("The Lich arbitrage bot is ready to start trading!");
    info!("Press Ctrl+C to stop the bot");

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl+c");

    info!("Shutdown signal received. Stopping The Lich...");

    // TODO: Graceful shutdown of all components

    info!("The Lich has been stopped successfully");
    Ok(())
}

fn init_logging() -> Result<()> {
    // Get log level from environment or default to info
    let log_level = std::env::var("LICH_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&log_level));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();

    Ok(())
}
