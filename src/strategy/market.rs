use futures_util::{SinkExt, StreamExt}; // ✅ [修复] 引入 SinkExt
use log::{info, error, warn, debug};
use tokio_tungstenite::tungstenite::Message;
use futures_util::stream::{SplitStream, SplitSink};
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;
use tokio_native_tls::TlsStream;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use crate::okx::protocol::{self, WsRouter, AccountData};
use crate::okx::market_data::Ticker;

type WsWriteStream = SplitSink<WebSocketStream<TlsStream<TcpStream>>, Message>;
type WsReadStream = SplitStream<WebSocketStream<TlsStream<TcpStream>>>;

pub struct StrategyState {
    pub usdt_balance: RwLock<f64>,
    pub is_locked: AtomicBool,
}

pub struct MarketStrategy {
    price_history: RwLock<HashMap<String, VecDeque<(i64, f64)>>>,
    state: Arc<StrategyState>,
}

impl MarketStrategy {
    pub fn new() -> Self {
        MarketStrategy {
            price_history: RwLock::new(HashMap::new()),
            state: Arc::new(StrategyState {
                usdt_balance: RwLock::new(0.0),
                is_locked: AtomicBool::new(false),
            }),
        }
    }

    /// 🚀 [策略主循环] 接收双通道数据
    /// ✅ 签名修正：接收 4 个参数，解决 main.rs 的调用错误
    pub async fn run(
        &self,
        mut read_pub: WsReadStream,
        mut write_pub: WsWriteStream,
        mut read_priv: WsReadStream,
        mut write_priv: WsWriteStream
    ) {
        info!("🧠 [狙击引擎] 监听中: Public(行情) + Private(交易)");

        let mut heartbeat_interval = tokio::time::interval(tokio::time::Duration::from_secs(15));

        loop {
            tokio::select! {
                // 1. 定时心跳
                _ = heartbeat_interval.tick() => {
                    // 两条连接都需要心跳保活
                    if let Err(_) = write_pub.send(Message::Text("ping".to_string())).await {}
                    if let Err(_) = write_priv.send(Message::Text("ping".to_string())).await {}
                }

                // 2. Public 消息 (行情)
                msg_res = read_pub.next() => {
                    if let Some(Ok(Message::Text(text))) = msg_res {
                        if text == "pong" { continue; }
                        // 收到行情 -> 分析 -> 可能通过 write_priv 下单
                        if let Some(order_json) = self.process_public_message(&text) {
                            info!("🔥 [触发下单] 发送指令...");
                            if let Err(e) = write_priv.send(Message::Text(order_json)).await {
                                error!("❌ [致命] 下单失败: {}", e);
                            }
                        }
                    }
                }

                // 3. Private 消息 (账户/订单)
                msg_res = read_priv.next() => {
                    if let Some(Ok(Message::Text(text))) = msg_res {
                        if text == "pong" { continue; }
                        self.process_private_message(&text);
                    }
                }
            }
        }
    }

    /// 处理行情消息 (Public)
    fn process_public_message(&self, text: &str) -> Option<String> {
        let router: WsRouter = match serde_json::from_str(text) {
            Ok(r) => r,
            Err(_) => return None,
        };

        // 处理订阅确认
        if let Some(event) = &router.event {
            if event == "error" {
                error!("❌ [Public Error] {:?}", router.msg);
            }
            return None;
        }

        // 处理 Ticker
        if let Some(arg) = router.arg {
            if arg.channel == "tickers" {
                if let Some(raw_data) = router.data {
                    if let Ok(tickers) = serde_json::from_str::<Vec<Ticker>>(raw_data.get()) {
                        for t in tickers {
                            // 🚀 核心分析
                            if let Some(order) = self.analyze_ticker(t) {
                                return Some(order);
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// 处理账户/交易消息 (Private)
    fn process_private_message(&self, text: &str) {
        let router: WsRouter = match serde_json::from_str(text) {
            Ok(r) => r,
            Err(_) => return,
        };

        // ✅ [优化] 移除了 redundant 的 login 判断
        // 因为 client.rs 已经确保了登录成功才会走到这里
        if let Some(event) = &router.event {
            if event == "error" {
                error!("❌ [Private Error] Code: {:?}, Msg: {:?}", router.code, router.msg);
            }
            return;
        }

        // 处理余额推送
        if let Some(arg) = router.arg {
            if arg.channel == "account" {
                self.update_balance(router.data.as_deref());
            }
        }
    }

    /// 🕵️ [分析逻辑]
    fn analyze_ticker(&self, ticker: Ticker) -> Option<String> {
        let inst_id = ticker.inst_id.clone();
        let price = ticker.last;
        let now = chrono::Utc::now().timestamp_millis();
        let exchange_ts = ticker.ts.parse::<i64>().unwrap_or(now);
        let latency = now - exchange_ts;

        if inst_id.contains("SWAP") { return None; }

        let mut history_map = self.price_history.write().unwrap();
        let queue = history_map.entry(inst_id.clone()).or_insert(VecDeque::with_capacity(50));
        queue.push_back((now, price));

        while let Some(front) = queue.front() {
            if now - front.0 > 5000 { queue.pop_front(); } else { break; }
        }

        if let Some(old_data) = queue.front() {
            let old_price = old_data.1;
            let change_pct = (price - old_price) / old_price;

            if change_pct.abs() > 0.001 {
                let sign = if change_pct > 0.0 { "+" } else { "" };
                info!("🌊 [波动] {} 2s幅: {}{:.2}% | 延迟: {}ms | 价格: {}",
                      inst_id, sign, change_pct * 100.0, latency, price);
            }

            if change_pct < -0.03 {
                info!("🚨 [狙击信号] {} 暴跌 {:.2}%", inst_id, change_pct * 100.0);
                if !self.state.is_locked.load(Ordering::SeqCst) {
                    let balance = *self.state.usdt_balance.read().unwrap();
                    if balance > 10.0 {
                        self.state.is_locked.store(true, Ordering::SeqCst);
                        warn!("🚀 [执行] 买入 {}, 金额: ${}", inst_id, balance);
                        return Some(protocol::create_order_packet(
                            &inst_id, "buy", &balance.to_string(), None
                        ));
                    }
                }
            }
        }
        None
    }

    fn update_balance(&self, data: Option<&serde_json::value::RawValue>) {
        if let Some(raw) = data {
            if let Ok(account_data) = serde_json::from_str::<Vec<AccountData>>(raw.get()) {
                if let Some(details) = account_data.first() {
                    for balance in &details.details {
                        if balance.ccy == "USDT" {
                            if let Ok(avail) = balance.avail_bal.parse::<f64>() {
                                let mut bal_lock = self.state.usdt_balance.write().unwrap();
                                *bal_lock = avail;
                                info!("💰 [账户同步] USDT: ${:.2}", avail);
                            }
                        }
                    }
                }
            }
        }
    }
}