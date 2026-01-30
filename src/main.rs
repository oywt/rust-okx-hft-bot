// 声明模块
mod config;
mod okx;
mod strategy;
mod utils;
use futures_util::{SinkExt, StreamExt};
use crate::config::AppConfig;
use crate::okx::client::OkxClient;
use crate::strategy::market::MarketStrategy;
use log::{info, error};
use crate::okx::protocol::{self, ChannelType, Endpoint};

#[tokio::main]
async fn main() {
    // 1. 初始化日志系统 (优先读取环境变量，默认 info 级别)
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("🚀 [启动] Rust HFT 高频交易机器人 v0.1");

    // 2. 加载配置
    let config = AppConfig::load();
    info!("📋 [配置] 模拟模式: {}, API_KEY: ...{}", config.simulation_mode, &config.okx_api_key[0..4]);

    // 3. 初始化 OKX 客户端
    let client = OkxClient::new(Endpoint::Public);

    // 4. 连接并登录 (这一步会发回一个 WebSocket 流)
    let ws_stream = match client.connect(&config).await {
        Some(s) => s,
        None => {
            error!("❌ [致命] 无法连接到 OKX (请检查代理配置)，程序退出。");
            return;
        }
    };

    // 5. 拆分流：读(Read) 和 写(Write)
    // Write流给发单引擎(TradeEngine)，Read流给策略引擎(StrategyEngine)
    let (mut write, read) = ws_stream.split();

    // ==========================================
    // 📡 [领域驱动] 发送订阅指令
    // ==========================================
    info!("📡 [指令] 正在构建订阅请求...");

    // 🏭 使用工厂生成指令包 (业务逻辑)
    let sub_msg = protocol::create_subscribe_packet(ChannelType::Tickers, "BTC-USDT");

    // 🚀 发射 (执行逻辑)
    if let Err(e) = write.send(sub_msg).await {
        error!("❌ [订阅] 发送失败: {}", e);
        return;
    }
    info!("✅ [指令] 订阅请求已发送 (Target: BTC-USDT)");



    // 注意：write 流暂时还没用，我们为了编译通过，先把它持有住或者丢弃
    // 在下一阶段，我们会把 write 传给一个 Sender 线程用来发单
    let _write_handle = write;

    // 6. 启动策略引擎 (接管 Read 流)
    let strategy = MarketStrategy::new();

    // await 会阻塞在这里，直到连接断开
    strategy.run(read).await;

    info!("👋 [退出] 主程序结束。");
}