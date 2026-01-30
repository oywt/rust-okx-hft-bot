use futures_util::StreamExt;
use log::{info, error, warn};
use tokio_tungstenite::tungstenite::Message;
use futures_util::stream::SplitStream;
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;
use tokio_native_tls::TlsStream;
use crate::utils::logger::LogFormatter;
use std::collections::HashMap;
use std::sync::RwLock;

use crate::okx::protocol::WsRouter;
use crate::okx::market_data::Ticker;


type WsReadStream = SplitStream<WebSocketStream<TlsStream<TcpStream>>>;

pub struct MarketStrategy {
    ticker_map: RwLock<HashMap<String, Ticker>>,
}

impl MarketStrategy {
    pub fn new() -> Self {
        MarketStrategy {
            ticker_map: RwLock::new(HashMap::new()),
        }
    }

    pub async fn run(&self, mut read: WsReadStream) {
        info!("🧠 [策略引擎] HFT 模式启动 (Zero-Copy Router)...");
        while let Some(msg_result) = read.next().await {
            match msg_result {
                Ok(Message::Text(text)) => self.dispatch(&text),
                Ok(Message::Ping(_)) => {},
                Err(e) => { error!("❌ WebSocket 中断: {}", e); break; }
                _ => {}
            }
        }
    }

    fn dispatch(&self, text: &str) {
        // 1. 零拷贝路由
        let router: WsRouter = match serde_json::from_str(text) {
            Ok(r) => r,
            Err(e) => {
                // 🛠️ 增强：如果最外层解析都失败了，打印原始文本方便调试
                error!("❌ [解析失败] 无法识别的消息格式: {} | Raw: {}", e, text);
                return;
            }
        };

        // 2. 业务处理
        if let Some(arg) = router.arg {
            if arg.channel == "tickers" {
                if let Some(raw_data) = router.data {
                    // RawValue -> Ticker (f64)
                    if let Ok(tickers) = serde_json::from_str::<Vec<Ticker>>(raw_data.get()) {
                        for t in tickers { self.on_market_ticker(t); }
                    }
                }
            }
        } else if let Some(event) = router.event {
            if event == "error" {
                error!("❌ OKX Error: {:?} {:?}", router.code, router.msg);
            }
        }
    }

    fn on_market_ticker(&self, ticker: Ticker) {
        // 1. ⏱️ 先计算延迟 (和 logger 里一样的逻辑)
        let remote_ts = ticker.ts.parse::<i64>().unwrap_or(0);
        let local_ts = chrono::Utc::now().timestamp_millis();
        let latency = local_ts - remote_ts;

        // 🛡️ [风控] 延迟熔断机制
        // 如果延迟超过 800ms (你可以根据实际网络情况调整，本地开发建议设高点比如 1000ms，生产环境设 100ms)
        if latency > 300 {
            // 记录一条警告日志，告诉自己这一跳数据废了
            warn!("⚠️ [风控] 丢弃过期数据! Latency: {}ms > 800ms | {}", latency, ticker.inst_id);
            // ❌ 直接返回，不更新状态，不触发下单！
            return;
        }

        // --- 只有数据“新鲜”，才继续往下走 ---

        let inst_id = ticker.inst_id.clone();

        // 2. 更新内存状态 (原子操作)
        {
            let mut map = self.ticker_map.write().unwrap();
            map.insert(inst_id.clone(), ticker.clone());
        }

        // 3. 打印日志 (组件化)
        // 注意：LogFormatter 里也会算一遍延迟，但这微不足道，为了解耦可以重复算
        let log_msg = crate::utils::logger::LogFormatter::format_ticker(&ticker);
        info!("{}", log_msg);

        // 4. 🚀 触发交易信号
        self.evaluate_signal(&inst_id);
    }

    fn evaluate_signal(&self, inst_id: &str) {
        // 🔒 获取读锁 (Read Lock)
        // 这里的开销极小，因为我们只需要读
        let map = self.ticker_map.read().unwrap();
        if let Some(ticker) = map.get(inst_id) {
            // 这里可以写真正的策略逻辑
            // 比如: RSI 计算, 布林带, 网格策略等
            // 目前我们什么都不做，因为日志已经在 on_market_ticker 里打印过了

            // 示例：如果价差变成负数（不可能发生，但作为逻辑测试），打印个错误
            if ticker.ask_px < ticker.bid_px {
                error!("❌ [严重错误] 市场倒挂: Ask < Bid ({})", inst_id);
            }
        }
    }

}
