use futures_util::{SinkExt, StreamExt};
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

// 类型别名简化代码
type WsWriteStream = SplitSink<WebSocketStream<TlsStream<TcpStream>>, Message>;
type WsReadStream = SplitStream<WebSocketStream<TlsStream<TcpStream>>>;

/// 🧠 [共享状态] 线程安全、原子性
pub struct StrategyState {
    /// 账户 USDT 余额 (读多写少，用 RwLock)
    pub usdt_balance: RwLock<f64>,
    /// 交易锁 (原子布尔值，无锁并发，纳秒级检查)
    /// true = 已上锁/已开单，禁止再次开单
    pub is_locked: AtomicBool,
}

pub struct MarketStrategy {
    /// 📊 [价格滑窗] RingBuffer
    /// Key: "DOGE-USDT"
    /// Value: 双端队列 [(ts, price), (ts, price)...]
    /// 作用: 存储最近 10秒 的价格，用于计算瞬时加速度
    price_history: RwLock<HashMap<String, VecDeque<(i64, f64)>>>,

    /// 共享状态 (余额、锁)
    state: Arc<StrategyState>,
}

impl MarketStrategy {
    pub fn new() -> Self {
        MarketStrategy {
            price_history: RwLock::new(HashMap::new()),
            state: Arc::new(StrategyState {
                usdt_balance: RwLock::new(0.0), // 初始余额 0
                is_locked: AtomicBool::new(false),
            }),
        }
    }

    /// 🚀 [主循环] 策略引擎启动
    /// 改动点：引入 select! 实现心跳保活
    pub async fn run(&self, mut read: WsReadStream, mut write: WsWriteStream) {
        info!("🧠 [狙击引擎] 启动 | 目标内部时延: <1ms | 策略: 暴跌接针");

        // ✅ [新增] 1. 心跳定时器 (每15秒一次)
        let mut heartbeat_interval = tokio::time::interval(tokio::time::Duration::from_secs(15));

        loop {
            tokio::select! {
                // ✅ [新增] 2. 定时发送心跳
                _ = heartbeat_interval.tick() => {
                    debug!("💓 [系统] 发送应用层心跳 Ping...");
                    // OKX 要求发送字符串 "ping"
                    if let Err(e) = write.send(Message::Text("ping".to_string())).await {
                        error!("❌ 心跳发送失败: {}", e);
                        break;
                    }
                }

                // 3. 处理接收到的消息
                msg_result = read.next() => {
                    match msg_result {
                        Some(Ok(msg)) => {
                            match msg {
                                Message::Text(text) => {
                                    // 忽略服务器回的 pong
                                    if text == "pong" { continue; }

                                    // 🔥 热路径 (Hot Path) 开始
                                    if let Some(order_json) = self.process_message(&text) {
                                        info!("🔥 [开火] 触发狙击! 发送指令: {}", order_json);

                                        // 立即写入网络缓冲区
                                        if let Err(e) = write.send(Message::Text(order_json)).await {
                                            error!("❌ [致命] 发送失败: {}", e);
                                        }
                                    }
                                },
                                Message::Ping(_) => {
                                    // 响应标准协议 Ping
                                    let _ = write.send(Message::Pong(vec![])).await;
                                },
                                Message::Close(_) => {
                                    warn!("⚠️ 服务器主动关闭连接");
                                    break;
                                },
                                _ => {}
                            }
                        },
                        Some(Err(e)) => {
                            error!("❌ WebSocket 连接错误: {}", e);
                            break;
                        },
                        None => {
                            warn!("⚠️ WebSocket 流结束");
                            break;
                        }
                    }
                }
            }
        }
    }

    /// ⚡ [决策中枢] 处理所有传入数据
    fn process_message(&self, text: &str) -> Option<String> {
        // ✅ [新增] 错误处理：如果解析失败，打印原始内容
        let router: WsRouter = match serde_json::from_str(text) {
            Ok(r) => r,
            Err(e) => {
                // 只有非 pong 消息解析失败才打印警告
                if text != "pong" {
                    warn!("⚠️ [解析失败] 无法识别的消息: {} | Raw: {}", e, text);
                }
                return None;
            },
        };

        if let Some(arg) = router.arg {
            // A. 行情推送 (Tickers) -> 进入分析引擎
            if arg.channel == "tickers" {
                if let Some(raw_data) = router.data {
                    // 解析 Ticker 数组
                    if let Ok(tickers) = serde_json::from_str::<Vec<Ticker>>(raw_data.get()) {
                        for t in tickers {
                            // 🔥 核心分析入口
                            if let Some(order) = self.analyze_ticker(t) {
                                return Some(order);
                            }
                        }
                    }
                }
            }
            // B. 账户推送 (Account) -> 更新本地余额缓存
            else if arg.channel == "account" {
                self.update_balance(router.data.as_deref());
            }
        }
        None
    }

    /// 🕵️ [精算师] 价格异动分析
    fn analyze_ticker(&self, ticker: Ticker) -> Option<String> {
        let inst_id = ticker.inst_id.clone();
        let price = ticker.last;
        let now = chrono::Utc::now().timestamp_millis();

        // ❌ 过滤: 我们只接现货的针 (排除 SWAP 合约)
        if inst_id.contains("SWAP") { return None; }
        // 解析 OKX 时间戳，计算链路延迟
        let exchange_ts = ticker.ts.parse::<i64>().unwrap_or(now);
        let latency = now - exchange_ts;

        // 获取或创建滑动窗口
        let mut history_map = self.price_history.write().unwrap();
        let queue = history_map.entry(inst_id.clone()).or_insert(VecDeque::with_capacity(50));

        // 1. 写入最新价格
        queue.push_back((now, price));

        // 2. 清理过期数据 (保留最近 5秒)
        while let Some(front) = queue.front() {
            if now - front.0 > 5000 {
                queue.pop_front();
            } else {
                break;
            }
        }

        // 3. 计算 "瞬时跌幅" (Velocity)
        // 寻找 2000ms (2秒) 前的价格作为基准
        if let Some(old_data) = queue.iter().find(|(ts, _)| now - ts >= 2000) {
            let old_price = old_data.1;

            // 📉 跌幅公式
            let drop_pct = (price - old_price) / old_price;

            // 逻辑: 只有波动 > 0.1% 才打印，过滤掉 90% 的无效刷屏
            if drop_pct.abs() > 0.001 {
                let sign = if drop_pct > 0.0 { "+" } else { "" };
                // 直接使用 info! 宏，不经过任何中间层 formatter
                info!("🌊 [波动] {} 2s幅: {}{:.2}% | 延迟: {}ms | 价格: {} -> {}",
                      inst_id, sign, drop_pct * 100.0, latency, old_price, price);
            }

            // 🎯 [触发阈值] 2秒内跌幅超过 3%
            // 这是一个非常激进的信号，代表恐慌盘涌出
            if drop_pct < -0.03 {
                info!("🚨 [异动捕捉] {} 2秒暴跌 {:.2}% | {} -> {}", inst_id, drop_pct * 100.0, old_price, price);

                // 4. 风控检查 (Atomic Check)
                // load(SeqCst) 是原子操作，极快
                if !self.state.is_locked.load(Ordering::SeqCst) {
                    let balance = *self.state.usdt_balance.read().unwrap();

                    // 最小下单金额保护 (防止余额不足报错)
                    // 假设最小 10 U
                    if balance > 10.0 {
                        // 🔒 立即上锁! (Compare-and-Swap 逻辑)
                        // 确保这一瞬间只有一个线程能进入这里，防止连发
                        self.state.is_locked.store(true, Ordering::SeqCst);

                        warn!("🚀 [狙击执行] 确认接飞刀! 目标: {}, 余额: ${}", inst_id, balance);

                        // 5. 生成极速下单指令
                        // 全仓买入 (balance to string)
                        return Some(protocol::create_order_packet(
                            &inst_id,
                            "buy",
                            &balance.to_string(),
                            None
                        ));
                    } else {
                        debug!("⚠️ [风控] 发现机会但余额不足: ${:.2}", balance);
                    }
                }
            }
        }

        None
    }

    /// 💰 更新本地余额缓存
    fn update_balance(&self, data: Option<&serde_json::value::RawValue>) {
        if let Some(raw) = data {
            // 解析 AccountData 结构
            if let Ok(account_data) = serde_json::from_str::<Vec<AccountData>>(raw.get()) {
                if let Some(details) = account_data.first() {
                    for balance in &details.details {
                        if balance.ccy == "USDT" {
                            if let Ok(avail) = balance.avail_bal.parse::<f64>() {
                                // 更新原子锁和余额
                                let mut bal_lock = self.state.usdt_balance.write().unwrap();
                                *bal_lock = avail;

                                // 💡 可选逻辑: 如果余额增加了(充值成功)，自动解锁
                                // self.state.is_locked.store(false, Ordering::SeqCst);

                                info!("💰 [账户同步] USDT 可用余额更新: ${:.2}", avail);
                            }
                        }
                    }
                }
            }
        }
    }
}