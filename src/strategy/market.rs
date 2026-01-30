use futures_util::StreamExt;
use log::{info, error};
use tokio_tungstenite::tungstenite::Message;
use futures_util::stream::SplitStream;
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;
use tokio_native_tls::TlsStream;

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
        let inst_id = ticker.inst_id.clone();
        {
            let mut map = self.ticker_map.write().unwrap();
            map.insert(inst_id.clone(), ticker.clone());
        }

        let spread = ticker.ask_px - ticker.bid_px;
        info!("⚡ [{}] Last: {:.2} | Spread: {:.2}", inst_id, ticker.last, spread);
    }
}
