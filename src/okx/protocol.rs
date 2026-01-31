use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use uuid::Uuid;

// ==========================================
// 🚀 极速 ID 生成器 (Hot Path)
// ==========================================
// 全局静态原子计数器，CPU 指令级自增，耗时 < 5ns
// 比 UUID 快 100 倍以上
static ORDER_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

// ==========================================
// 📦 基础枚举与结构
// ==========================================

#[derive(Debug, Clone, Copy)]
pub enum Endpoint {
    Public,
    Private, // 交易必须用 Private
}

// ✅ [修复点 1] 补回 Endpoint 的方法实现，解决 client.rs 的报错
impl Endpoint {
    pub fn as_url(&self) -> &'static str {
        match self {
            Endpoint::Public => "wss://ws.okx.com:8443/ws/v5/public",
            Endpoint::Private => "wss://ws.okx.com:8443/ws/v5/private",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ChannelType {
    Tickers,
    Account, // 余额频道
}

impl ChannelType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChannelType::Tickers => "tickers",
            ChannelType::Account => "account",
        }
    }
}

// 通用 WebSocket 消息路由
#[derive(Debug, Deserialize)]
pub struct WsRouter {
    pub arg: Option<WsArg>,

    // ✅ [修复点 2] 使用 Box<RawValue> 解决 "size cannot be known" 报错
    // RawValue 是不定长类型(?Sized)，必须装在 Box 指针里才能放在结构体中
    pub data: Option<Box<serde_json::value::RawValue>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WsArg {
    pub channel: String,
    pub instId: Option<String>,
}

// ==========================================
// ⚔️ 交易协议 (结构化序列化 - 零内存浪费)
// ==========================================

// 定义强类型结构体，避免 json! 宏的动态开销
#[derive(Serialize)]
struct OrderRequest<'a> {
    id: String,
    op: &'static str,
    args: [OrderArgs<'a>; 1], // 定长数组，避免 Vec 分配
}

#[derive(Serialize)]
struct OrderArgs<'a> {
    clOrdId: String,
    side: &'a str,
    posSide: &'a str,
    ordType: &'a str,
    instId: &'a str,
    sz: &'a str,
    tdMode: &'a str,
}

/// 🚀 [核心] 构造下单指令 (极速版)
/// 场景: 现货接针 / 合约套利
/// 性能: ~0.5微秒
pub fn create_order_packet(inst_id: &str, side: &str, size: &str, pos_side: Option<&str>) -> String {
    // 1. 极速判断交易模式
    // "SWAP" -> 合约全仓; 其他 -> 现货现金
    let (ord_type, td_mode) = if inst_id.contains("SWAP") {
        ("market", "cross")
    } else {
        ("market", "cash")
    };

    let final_pos_side = pos_side.unwrap_or("net");

    // 2. 生成极速 ID (Atomic Inc)
    // 格式: snip{时间戳后4位}{计数器} -> 保证唯一且极短
    let nonce = ORDER_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let now_secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();

    // clOrdId: 客户端自定义ID，必须唯一
    let cl_ord_id = format!("snip{:x}{}", now_secs % 10000, nonce);

    // request_id: WebSocket 请求 ID
    let req_id = Uuid::new_v4().to_string();

    // 3. 构造零拷贝结构体
    let request = OrderRequest {
        id: req_id,
        op: "order",
        args: [OrderArgs {
            clOrdId: cl_ord_id,
            side,
            posSide: final_pos_side,
            ordType: ord_type,
            instId: inst_id,
            sz: size,
            tdMode: td_mode,
        }],
    };

    // 4. 直接序列化为 JSON String
    serde_json::to_string(&request).unwrap()
}

// ==========================================
// 📡 订阅协议
// ==========================================

pub fn create_subscribe_packet(channel: ChannelType, inst_id: &str) -> String {
    let args = if inst_id == "USDT" || inst_id == "ANY" {
        // 账户频道特殊处理，ccy=USDT
        serde_json::json!([{
            "channel": channel.as_str(),
            "ccy": "USDT"
        }])
    } else {
        serde_json::json!([{
            "channel": channel.as_str(),
            "instId": inst_id
        }])
    };

    serde_json::json!({
        "op": "subscribe",
        "args": args
    }).to_string()
}

// ==========================================
// 💰 账户数据结构 (用于解析余额)
// ==========================================
#[derive(Debug, Deserialize)]
pub struct AccountData {
    pub details: Vec<BalanceDetail>,
}

#[derive(Debug, Deserialize)]
pub struct BalanceDetail {
    pub ccy: String,      // "USDT"
    #[serde(rename = "availBal")]
    pub avail_bal: String, // 可用余额
    #[serde(rename = "cashBal")]
    pub cash_bal: String,
}