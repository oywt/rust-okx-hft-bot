use futures_util::StreamExt;
use log::{info, error, warn};
use tokio_tungstenite::tungstenite::Message;
use futures_util::stream::SplitStream;
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;

// [新增] 引入 TLS 流类型 (因为 client.rs 现在传过来的是强制加密流)
use tokio_native_tls::TlsStream;

// [修改] 关键修复：把 MaybeTlsStream 改成 TlsStream<TcpStream>
// 这样就和 main.rs 里传进来的流类型完全对齐了
type WsReadStream = SplitStream<WebSocketStream<TlsStream<TcpStream>>>;

pub struct MarketStrategy {
    // 这里未来可以放一些状态，比如当前的持仓、目标价格等
}

impl MarketStrategy {
    pub fn new() -> Self {
        MarketStrategy {}
    }

    /// 启动策略循环
    pub async fn run(&self, mut read: WsReadStream) {
        info!("🧠 [策略] 市场监控引擎已启动，正在监听数据...");

        while let Some(msg_result) = read.next().await {
            match msg_result {
                Ok(Message::Text(text)) => {
                    // 这里是处理文本消息的核心逻辑
                    self.handle_message(&text);
                }
                Ok(Message::Ping(_)) => {
                    // Tungstenite 库会自动处理 Pong，不需要手动回复
                }
                Err(e) => {
                    error!("❌ [网络] WebSocket 读取错误: {}", e);
                    break; // 出错退出循环
                }
                _ => {}
            }
        }
        warn!("🛑 [策略] WebSocket 连接已断开，循环结束。");
    }

    /// 处理具体的 JSON 消息
    fn handle_message(&self, text: &str) {
        // 简单验证登录是否成功
        if text.contains("login") && text.contains("0") {
            info!("✅ [OKX] 登录验证成功！权限已解锁。");
        } else if text.contains("error") {
            error!("❌ [OKX] 收到错误消息: {}", text);
        } else {
            info!("📩 [数据] 收到推送: {}", text);
        }
    }
}