// src/main.rs
use crate::config::AppConfig;
use crate::okx::client::{OkxClient, Endpoint};
use crate::okx::protocol::{self, ChannelType};
use crate::strategy::market::MarketStrategy;
use futures_util::{SinkExt, StreamExt};
use log::{info, error};
use tokio_tungstenite::tungstenite::Message;

mod config;
mod okx;
mod strategy;
pub mod utils; // ✅ 确保这行存在

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("==================================================");
    info!("🏴‍☠️  Rust HFT Sniper Bot v1.0 [Profit First]");
    let config = AppConfig::load();

    // 1. 行情连接
    let client_pub = OkxClient::new(Endpoint::Public);
    let ws_pub = client_pub.connect(&config).await.unwrap(); // 偷懒unwrap，如果挂了直接panic重启
    let (mut write_pub, read_pub) = ws_pub.split();

    // 订阅列表 (10个精选)
    let watchlist = vec!["WIF-USDT", "PEPE-USDT", "BONK-USDT", "DOGE-USDT", "SOL-USDT", "JUP-USDT", "WLD-USDT", "ORDI-USDT", "SUI-USDT", "NEAR-USDT"];

    for inst_id in watchlist {
        let sub = protocol::create_subscribe_packet(ChannelType::Tickers, inst_id);
        write_pub.send(Message::Text(sub)).await.unwrap();
    }

    // 2. 交易连接
    let client_priv = OkxClient::new(Endpoint::Private);
    let ws_priv = client_priv.connect(&config).await.unwrap();
    let (mut write_priv, read_priv) = ws_priv.split();

    let sub_acc = protocol::create_subscribe_packet(ChannelType::Account, "USDT");
    write_priv.send(Message::Text(sub_acc)).await.unwrap();

    // 3. 启动
    let strategy = MarketStrategy::new();
    strategy.run(read_pub, write_pub, read_priv, write_priv).await;
}
