# The Lich - Solana Arbitrage Bot

The Lich is a high-speed arbitrage bot designed for the Solana blockchain that identifies and executes profitable trading opportunities across multiple DEXs with minimal latency.

## Features

- **Multi-DEX Support**: Supports major Solana DEXs including Raydium, Orca, Jupiter, and more
- **Real-time Price Monitoring**: WebSocket-based price feeds for ultra-low latency
- **Advanced Arbitrage Detection**: Simple and triangular arbitrage opportunity detection
- **Risk Management**: Configurable profit thresholds, slippage tolerance, and position sizing
- **High Performance**: Built in Rust for maximum speed and efficiency
- **Comprehensive Monitoring**: Detailed logging, metrics, and health monitoring
- **Flexible Configuration**: TOML-based configuration with environment variable support

## Architecture

The bot consists of several key components:

### Core Modules

1. **Network Layer** (`src/network/`)
   - RPC client management with failover
   - WebSocket connections for real-time data
   - Health monitoring and connection management

2. **Data Management** (`src/data/`)
   - Price feed aggregation from multiple sources
   - Order book management
   - Pool data collection and normalization
   - Market data aggregation

3. **Arbitrage Engine** (`src/arbitrage/`)
   - Opportunity detection algorithms
   - Profit calculation and risk assessment
   - Strategy management and execution logic

4. **Transaction Management** (`src/transaction/`)
   - Transaction building and optimization
   - Execution with MEV protection
   - Confirmation tracking

5. **Configuration** (`src/config/`)
   - DEX configurations
   - Token metadata management
   - Wallet and security settings

## Quick Start

### Prerequisites

- Rust 1.70+ installed
- Solana CLI tools (optional, for wallet management)
- Access to Solana RPC endpoints (Helius, QuickNode, etc.)

### Installation

1. Clone the repository:
```bash
git clone https://github.com/your-username/the-lich-1.git
cd the-lich-1
```

2. Build the project:
```bash
cargo build --release
```

3. Copy and configure the settings:
```bash
cp config/default.toml config/local.toml
# Edit config/local.toml with your settings
```

4. Set up your wallet:
```bash
# Option 1: Environment variable (recommended)
export LICH_PRIVATE_KEY="your-base58-private-key"

# Option 2: Wallet file
# Place your Solana CLI wallet file and update config
```

### Configuration

The bot uses a hierarchical configuration system:

1. `config/default.toml` - Default settings
2. `config/{environment}.toml` - Environment-specific settings
3. `config/local.toml` - Local overrides (not committed)
4. Environment variables with `LICH_` prefix

Key configuration sections:

```toml
[bot]
enabled = true
environment = "development"

[network]
primary_rpc = ["https://api.mainnet-beta.solana.com"]
commitment = "confirmed"

[strategy]
min_profit_threshold = "0.001"  # 0.1%
max_slippage_tolerance = "0.002"  # 0.2%
default_trade_amount = "1000"

[wallet]
wallet_type = "Environment"
env_var_name = "LICH_PRIVATE_KEY"

[[strategy.monitored_pairs]]
base = "SOL"
quote = "USDC"
```

### Running the Bot

```bash
# Development mode
cargo run

# Production mode
cargo run --release

# With custom config
LICH_ENV=production cargo run --release

# With debug logging
LICH_LOG_LEVEL=debug cargo run
```

## Safety and Risk Management

⚠️ **Important Safety Notes:**

1. **Start Small**: Begin with small trade amounts to test the system
2. **Monitor Closely**: Always monitor the bot's performance and logs
3. **Set Limits**: Configure appropriate profit thresholds and position limits
4. **Secure Keys**: Never commit private keys to version control
5. **Test First**: Use devnet/testnet for initial testing

### Risk Controls

- Configurable maximum daily loss limits
- Position size limits as percentage of total balance
- Minimum SOL balance maintenance for fees
- Kill switch for emergency stops
- Comprehensive error handling and recovery

## Development

### Project Structure

```
src/
├── lib.rs              # Main library entry point
├── main.rs             # Binary entry point
├── config/             # Configuration management
├── network/            # Network layer (RPC, WebSocket)
├── data/               # Data collection and management
├── arbitrage/          # Arbitrage detection and analysis
├── transaction/        # Transaction building and execution
├── monitoring/         # Logging and metrics
└── error/              # Error types and handling
```

### Building and Testing

```bash
# Check code
cargo check

# Run tests
cargo test

# Run with warnings
cargo clippy

# Format code
cargo fmt

# Build documentation
cargo doc --open
```

## Performance Optimization

The bot is optimized for high-frequency trading:

- **Rust Performance**: Zero-cost abstractions and memory safety
- **Async Architecture**: Non-blocking I/O for maximum throughput
- **Connection Pooling**: Efficient RPC connection management
- **Data Caching**: In-memory caching of frequently accessed data
- **Batch Processing**: Efficient batch operations where possible

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality
5. Ensure all tests pass
6. Submit a pull request

## License

This project is licensed under the MIT License.

## Disclaimer

This software is provided for educational and research purposes. Trading cryptocurrencies involves substantial risk of loss. The authors are not responsible for any financial losses incurred through the use of this software. Always do your own research and trade responsibly.

---

**The Lich** - *"Death is only the beginning of profit."*
