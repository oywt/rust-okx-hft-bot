use crate::config::AppConfig;
use crate::okx::client::{OkxClient, Endpoint};
use crate::okx::protocol::{self, ChannelType};
use crate::strategy::market::MarketStrategy;
// ✅ [修复] 必须引入 SinkExt 才能调用 .send()，必须引入 StreamExt 才能调用 .next()
use futures_util::{SinkExt, StreamExt};
use log::{info, error};
use tokio_tungstenite::tungstenite::Message;

mod config;
mod okx;
mod strategy;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("==================================================");
    info!("🏴‍☠️  Rust HFT Sniper Bot v0.5 [Final Fix]");
    info!("==================================================");

    let config = AppConfig::load();

    // -----------------------------------------------------------
    // 🔗 1. Public 连接 (只听行情)
    // -----------------------------------------------------------
    info!("🔗 [1/2] 连接 Public 频道 (行情)...");
    let client_pub = OkxClient::new(Endpoint::Public);
    let ws_pub = match client_pub.connect(&config).await {
        Some(s) => s,
        None => return,
    };
    let (mut write_pub, read_pub) = ws_pub.split();

    // 订阅行情
    let watchlist = vec![
        // --- 👑 核心主流 ---
        "BTC-USDT", "ETH-USDT", "SOL-USDT", "BNB-USDT",
        // --- 🐕 活跃 Meme ---
        "DOGE-USDT", "PEPE-USDT", "SHIB-USDT", "BONK-USDT", "WIF-USDT", "FLOKI-USDT", "MEME-USDT", "BOME-USDT",
        // --- 📜 铭文 ---
        "ORDI-USDT", "SATS-USDT",
        // ❌ 移除 RATS (报错)

        // --- 🤖 AI & Layer 1/2 ---
        "RENDER-USDT", // ✅ 修正: RNDR -> RENDER
        "WLD-USDT", "FET-USDT",
        // ❌ 移除 TAO (报错)
        "AR-USDT", "FIL-USDT",

        "SUI-USDT", "SEI-USDT", "APT-USDT", "ARB-USDT", "OP-USDT", "TIA-USDT", "AVAX-USDT", "NEAR-USDT",
        "POL-USDT",    // ✅ 修正: MATIC -> POL
        "DOT-USDT", "ADA-USDT", "TRX-USDT", "LINK-USDT",

        // --- 🎢 老牌/热门 ---
        "XRP-USDT", "LTC-USDT", "BCH-USDT", "ETC-USDT",
        // ❌ 移除 EOS (报错)

        "JUP-USDT", "PYTH-USDT", "BLUR-USDT", "DYDX-USDT", "IMX-USDT", "LDO-USDT", "INJ-USDT", "ATOM-USDT"
    ];
    info!("📡 [Public] 订阅 {} 个目标...", watchlist.len());

    for chunk in watchlist.chunks(10) {
        for inst_id in chunk {
            let sub = protocol::create_subscribe_packet(ChannelType::Tickers, inst_id);
            if let Err(e) = write_pub.send(Message::Text(sub)).await {
                error!("❌ 订阅发送失败: {}", e);
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    // -----------------------------------------------------------
    // 🔗 2. Private 连接 (只做交易)
    // -----------------------------------------------------------
    info!("🔗 [2/2] 连接 Private 频道 (交易)...");
    let client_priv = OkxClient::new(Endpoint::Private);
    // client.connect 内部已完成登录鉴权
    let ws_priv = match client_priv.connect(&config).await {
        Some(s) => s,
        None => return,
    };
    let (mut write_priv, read_priv) = ws_priv.split();

    // 订阅账户
    let sub_acc = protocol::create_subscribe_packet(ChannelType::Account, "USDT");
    if let Err(e) = write_priv.send(Message::Text(sub_acc)).await {
        error!("❌ 账户订阅失败: {}", e);
    }

    info!("✅ [双线就绪] 策略引擎启动...");

    // -----------------------------------------------------------
    // 🧠 3. 策略引擎
    // -----------------------------------------------------------
    let strategy = MarketStrategy::new();
    // ✅ 传入 4 个参数，对应 market.rs 的新签名
    strategy.run(read_pub, write_pub, read_priv, write_priv).await;
}