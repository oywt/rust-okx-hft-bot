// 声明模块
mod config;
mod okx;
mod strategy;

pub mod utils;

use futures_util::{SinkExt, StreamExt};
use crate::config::AppConfig;
use crate::okx::client::OkxClient;
use crate::strategy::market::MarketStrategy;
use log::{info, error};
use crate::okx::protocol::{self, ChannelType, Endpoint};

#[tokio::main]
async fn main() {
    // 1. 初始化日志
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("🚀 [启动] Rust HFT 高频交易机器人 v0.2 (DDD Refactored)");

    // 2. 加载配置
    let config = AppConfig::load();

    // 3. 初始化 OKX 客户端 (使用 Public 演示行情，如需交易请改为 Private 并配置 key)
    let client = OkxClient::new(Endpoint::Public);

    // 4. 连接
    let ws_stream = match client.connect(&config).await {
        Some(s) => s,
        None => {
            error!("❌ [致命] 无法连接到 OKX，程序退出。");
            return;
        }
    };

    // 5. 拆分流
    let (mut write, read) = ws_stream.split();

    // ==========================================
    // 📡 [批量订阅] 支持多币种
    // ==========================================
    // 定义我们需要监听的投资组合
    let portfolio = vec!["BTC-USDT", "ETH-USDT", "SOL-USDT"];

    info!("📡 [指令] 正在批量订阅行情: {:?}", portfolio);

    for inst_id in portfolio {
        let sub_msg = protocol::create_subscribe_packet(ChannelType::Tickers, inst_id);
        if let Err(e) = write.send(sub_msg).await {
            error!("❌ [订阅] {} 发送失败: {}", inst_id, e);
        } else {
            // 简单的流控，防止发包过快被断开 (Optional)
            // tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
    }
    info!("✅ [指令] 订阅请求全部发送完毕。");

    // 持有 write 流，未来用于发单
    let _write_handle = write;

    // 6. 启动策略引擎
    let strategy = MarketStrategy::new();

    // 进入死循环，等待 WebSocket 数据
    strategy.run(read).await;

    info!("👋 [退出] 主程序结束。");
}
