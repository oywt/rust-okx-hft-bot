use futures_util::{SinkExt, StreamExt};
use log::{info, error, warn};
use tokio_tungstenite::tungstenite::Message;
use futures_util::stream::{SplitStream, SplitSink};
use tokio::net::TcpStream;
use tokio_tungstenite::WebSocketStream;
use tokio_native_tls::TlsStream;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use crate::okx::protocol::{self, WsRouter, AccountData};
use crate::okx::market_data::Ticker;
use crate::utils::logger::LogFormatter;

type WsWriteStream = SplitSink<WebSocketStream<TlsStream<TcpStream>>, Message>;
type WsReadStream = SplitStream<WebSocketStream<TlsStream<TcpStream>>>;

// ⚙️ 策略核心参数 (Strategy Config)
const ROUND_TRIP_COST: f64 = 0.004; // 0.4% 硬成本 (含滑点)
const BUY_CRASH_THRESHOLD: f64 = -0.025; // 5s跌幅 > 2.5% 才买
const TAKE_PROFIT_NET: f64 = 0.01; // 净赚 > 1.0% 才卖
const STOP_LOSS_NET: f64 = -0.03; // 净亏 > 3.0% 止损
const BET_SIZE_USDT: f64 = 25.0; // 单笔 25 U
const MAX_POSITIONS: usize = 3; // 最大持仓数

#[derive(Debug, Clone)]
struct Position {
    inst_id: String,
    entry_price: f64, // 必须是 Ask1 (实际买入成本)
    entry_ts: i64,
}

pub struct StrategyState {
    pub usdt_balance: RwLock<f64>,
    positions: RwLock<HashMap<String, Position>>,
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
                positions: RwLock::new(HashMap::new()),
            }),
        }
    }

    pub async fn run(
        &self,
        mut read_pub: WsReadStream,
        mut write_pub: WsWriteStream,
        mut read_priv: WsReadStream,
        mut write_priv: WsWriteStream
    ) {
        info!("🧠 [狙击引擎] Flash Crash Sniper 启动 | 费率风控: 开 | 精度: Ask/Bid");

        let mut heartbeat_interval = tokio::time::interval(tokio::time::Duration::from_secs(15));

        loop {
            tokio::select! {
                // 心跳
                _ = heartbeat_interval.tick() => {
                    let _ = write_pub.send(Message::Text("ping".to_string())).await;
                    let _ = write_priv.send(Message::Text("ping".to_string())).await;
                }
                // 行情消息
                msg_res = read_pub.next() => {
                    if let Some(Ok(Message::Text(text))) = msg_res {
                        if text == "pong" { continue; }
                        if let Some(order_json) = self.process_public_message(&text) {
                            if let Err(e) = write_priv.send(Message::Text(order_json)).await {
                                error!("❌ 下单失败: {}", e);
                            }
                        }
                    }
                }
                // 账户消息
                msg_res = read_priv.next() => {
                    if let Some(Ok(Message::Text(text))) = msg_res {
                        if text == "pong" { continue; }
                        self.process_private_message(&text);
                    }
                }
            }
        }
    }

    fn process_public_message(&self, text: &str) -> Option<String> {
        let router: WsRouter = match serde_json::from_str(text) {
            Ok(r) => r,
            Err(_) => return None,
        };
        if let Some(arg) = router.arg {
            if arg.channel == "tickers" {
                if let Some(raw_data) = router.data {
                    if let Ok(tickers) = serde_json::from_str::<Vec<Ticker>>(raw_data.get()) {
                        for t in tickers {
                            if let Some(order) = self.analyze_ticker(t) { return Some(order); }
                        }
                    }
                }
            }
        }
        None
    }

    fn process_private_message(&self, text: &str) {
        let router: WsRouter = match serde_json::from_str(text) {
            Ok(r) => r,
            Err(_) => return,
        };
        if let Some(arg) = router.arg {
            if arg.channel == "account" {
                self.update_balance(router.data.as_deref());
            }
        }
    }

    // 🕵️ [核心逻辑]
    fn analyze_ticker(&self, ticker: Ticker) -> Option<String> {
        let inst_id = ticker.inst_id.clone();

        // 🎯 [精确价格]
        // 判断趋势用 Last (反应快)
        // 计算成本用 Ask1 (买入价) / Bid1 (卖出价)
        let last_price = ticker.last;
        let buy_cost_price = ticker.ask_px;
        let sell_revenue_price = ticker.bid_px;

        let now = chrono::Utc::now().timestamp_millis();

        let log_msg = LogFormatter::format_ticker(&ticker);
        info!("{}", log_msg);

        // 延迟风控
        let remote_ts = ticker.ts.parse::<i64>().unwrap_or(0);
        if now - remote_ts > 2000 { return None; }

        // 1. 卖出逻辑 (如果有持仓)
        {
            let mut pos_map = self.state.positions.write().unwrap();

            if let Some(pos) = pos_map.get(&inst_id) {
                // 计算利润: (当前卖一价 - 当初买一价) / 当初买一价
                let gross_profit = (sell_revenue_price - pos.entry_price) / pos.entry_price;
                let net_profit = gross_profit - ROUND_TRIP_COST;

                // 止盈
                if net_profit > TAKE_PROFIT_NET {
                    warn!("💎 [止盈] {} 净赚 {:.2}% | 卖价: {}", inst_id, net_profit*100.0, sell_revenue_price);
                    pos_map.remove(&inst_id);
                    return Some(protocol::create_order_packet(&inst_id, "sell", "0", None));
                }
                // 止损
                if net_profit < STOP_LOSS_NET {
                    error!("🩸 [止损] {} 净亏 {:.2}% | 卖价: {}", inst_id, net_profit*100.0, sell_revenue_price);
                    pos_map.remove(&inst_id);
                    return Some(protocol::create_order_packet(&inst_id, "sell", "0", None));
                }
                // 超时 (10分钟)
                if now - pos.entry_ts > 600_000 {
                    warn!("⏰ [超时] {} 平仓", inst_id);
                    pos_map.remove(&inst_id);
                    return Some(protocol::create_order_packet(&inst_id, "sell", "0", None));
                }
                return None;
            }
            if pos_map.len() >= MAX_POSITIONS { return None; }
        }

        // 2. 买入逻辑 (如果没持仓)
        let mut history_map = self.price_history.write().unwrap();
        let queue = history_map.entry(inst_id.clone()).or_insert(VecDeque::with_capacity(20));
        // 记录 Last 价格用于判断趋势
        queue.push_back((now, last_price));

        while let Some(front) = queue.front() {
            if now - front.0 > 5000 { queue.pop_front(); } else { break; }
        }

        if let Some(old_data) = queue.front() {
            let old_price = old_data.1;
            // 跌幅计算依然用 Last (更能反映市场恐慌)
            let change_pct = (last_price - old_price) / old_price;

            if change_pct < BUY_CRASH_THRESHOLD {
                info!("📉 [暴跌侦测] {} 5s跌幅 {:.2}%", inst_id, change_pct * 100.0);

                let balance = *self.state.usdt_balance.read().unwrap();

                if balance >= BET_SIZE_USDT {
                    {
                        let mut pos_map = self.state.positions.write().unwrap();
                        // ✅ [修正] 记录持仓成本时，必须记录 buy_cost_price (Ask1)
                        // 这样后续计算盈亏才是真实的
                        pos_map.insert(inst_id.clone(), Position {
                            inst_id: inst_id.clone(),
                            entry_price: buy_cost_price,
                            entry_ts: now,
                        });
                    }
                    warn!("🚀 [狙击] 锁定 Ask1: {} | Last: {}", buy_cost_price, last_price);

                    return Some(protocol::create_order_packet(
                        &inst_id, "buy", &BET_SIZE_USDT.to_string(), None
                    ));
                }
            }
        }
        None
    }

    fn update_balance(&self, data: Option<&serde_json::value::RawValue>) {
        if let Some(raw) = data {
            if let Ok(acc) = serde_json::from_str::<Vec<AccountData>>(raw.get()) {
                if let Some(d) = acc.first() {
                    for b in &d.details {
                        if b.ccy == "USDT" {
                            if let Ok(v) = b.avail_bal.parse::<f64>() {
                                *self.state.usdt_balance.write().unwrap() = v;
                                info!("💰 [余额] USDT: ${:.2}", v);
                            }
                        }
                    }
                }
            }
        }
    }
}
