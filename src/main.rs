use crate::config::AppConfig;
use crate::okx::client::{OkxClient, Endpoint};
use crate::okx::protocol::{self, ChannelType};
use crate::strategy::market::MarketStrategy;
use futures_util::{SinkExt, StreamExt};
use log::info;
// ✅ [修正 1] 引入 Message 类型，用于包装发送的数据
use tokio_tungstenite::tungstenite::Message;

// 引入模块
mod config;
mod okx;
mod strategy;

#[tokio::main]
async fn main() {
    // 1. 初始化日志
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("==================================================");
    info!("🏴‍☠️  Rust HFT Sniper Bot v0.3 [Massive Scan Edition]");
    info!("🚀  Target: Top 60+ Volatile Assets");
    info!("==================================================");

    let config = AppConfig::load();

    info!("🔗 正在连接 OKX Private WebSocket...");
    let client = OkxClient::new(Endpoint::Private);

    let ws_stream = match client.connect(&config).await {
        Some(s) => s,
        None => {
            info!("❌ 连接失败，请检查 API Key 和 网络配置");
            return;
        }
    };

    let (mut write, read) = ws_stream.split();

    // 2. 🎯 [全市场选品] 狙击手目标清单
    let watchlist = vec![
        "BTC-USDT", "ETH-USDT", "SOL-USDT", "BNB-USDT",
        "DOGE-USDT", "PEPE-USDT", "SHIB-USDT", "BONK-USDT", "WIF-USDT", "FLOKI-USDT", "MEME-USDT", "BOME-USDT",
        "ORDI-USDT", "SATS-USDT", "RATS-USDT",
        "RNDR-USDT", "WLD-USDT", "FET-USDT", "TAO-USDT", "AR-USDT", "FIL-USDT",
        "SUI-USDT", "SEI-USDT", "APT-USDT", "ARB-USDT", "OP-USDT", "TIA-USDT", "AVAX-USDT", "NEAR-USDT", "MATIC-USDT", "DOT-USDT", "ADA-USDT", "TRX-USDT", "LINK-USDT",
        "XRP-USDT", "LTC-USDT", "BCH-USDT", "ETC-USDT", "EOS-USDT", "FIL-USDT",
        "JUP-USDT", "PYTH-USDT", "BLUR-USDT", "DYDX-USDT", "IMX-USDT", "LDO-USDT", "INJ-USDT", "ATOM-USDT"
    ];

    info!("📡 [全域雷达] 正在锁定 {} 个高波动目标...", watchlist.len());

    // 3. 🛡️ [战术分批订阅]
    let batch_size = 10;
    for (i, chunk) in watchlist.chunks(batch_size).enumerate() {
        info!("📡 发送第 {} 批订阅指令 ({} 个)...", i + 1, chunk.len());

        for inst_id in chunk {
            let sub = protocol::create_subscribe_packet(ChannelType::Tickers, inst_id);
            // ✅ [修正 2] 包装成 Message::Text
            write.send(Message::Text(sub)).await.unwrap();
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // 4. 订阅账户余额 (Account)
    let sub_acc = protocol::create_subscribe_packet(ChannelType::Account, "USDT");
    // ✅ [修正 3] 包装成 Message::Text
    write.send(Message::Text(sub_acc)).await.unwrap();

    info!("✅ [系统就绪] 全市场扫描已激活，等待任意标的暴跌 > 3% ...");

    // 5. 移交控制权
    let strategy = MarketStrategy::new();
    strategy.run(read, write).await;
}