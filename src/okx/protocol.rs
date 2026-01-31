use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use uuid::Uuid;

// ==========================================
// 🚀 极速 ID 生成器 (Hot Path)
// ==========================================
static ORDER_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

// ==========================================
// 📦 基础枚举与结构
// ==========================================

#[derive(Debug, Clone, Copy)]
pub enum Endpoint {
    Public,
    Private,
}

impl Endpoint {
    pub fn as_url(&self) -> &'static str {
        match self {
            Endpoint::Public => "wss://ws.okx.com/ws/v5/public",
            Endpoint::Private => "wss://ws.okx.com/ws/v5/private",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ChannelType {
    Tickers,
    Account,
}

impl ChannelType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChannelType::Tickers => "tickers",
            ChannelType::Account => "account",
        }
    }
}

// ✅ [修复核心] 补全字段，解决 market.rs 的编译报错
#[derive(Debug, Deserialize)]
pub struct WsRouter {
    pub arg: Option<WsArg>,

    // 🔔 新增: 系统事件字段 (login, subscribe, error)
    pub event: Option<String>,
    // 🔔 新增: 错误码
    pub code: Option<String>,
    pub msg: Option<String>,

    // 保持使用 Box 指针解决 size unknown 问题
    pub data: Option<Box<serde_json::value::RawValue>>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Serialize)]
pub struct WsArg {
    pub channel: String,

    #[serde(rename = "instId")]
    pub inst_id: Option<String>,
    pub instId: Option<String>,
    pub ccy: Option<String>,
}

// ==========================================
// ⚔️ 交易协议
// ==========================================

#[derive(Serialize)]
#[allow(non_snake_case)]
struct OrderRequest<'a> {
    id: String,
    op: &'static str,
    args: [OrderArgs<'a>; 1],
}

#[derive(Serialize)]
#[allow(non_snake_case)]
struct OrderArgs<'a> {
    clOrdId: String,
    side: &'a str,
    posSide: &'a str,
    ordType: &'a str,
    instId: &'a str,
    sz: &'a str,
    tdMode: &'a str,
}

pub fn create_order_packet(inst_id: &str, side: &str, size: &str, pos_side: Option<&str>) -> String {
    let (ord_type, td_mode) = if inst_id.contains("SWAP") {
        ("market", "cross")
    } else {
        ("market", "cash")
    };

    let final_pos_side = pos_side.unwrap_or("net");
    let nonce = ORDER_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let now_secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let cl_ord_id = format!("snip{:x}{}", now_secs % 10000, nonce);
    let req_id = Uuid::new_v4().to_string();

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

    serde_json::to_string(&request).unwrap()
}

pub fn create_subscribe_packet(channel: ChannelType, inst_id: &str) -> String {
    if channel.as_str() == "account" {
        serde_json::json!({
            "op": "subscribe",
            "args": [{
                "channel": channel.as_str(),
                "ccy": "USDT"
            }]
        }).to_string()
    } else {
        serde_json::json!({
            "op": "subscribe",
            "args": [{
                "channel": channel.as_str(),
                "instId": inst_id
            }]
        }).to_string()
    }
}

// ==========================================
// 💰 账户数据结构
// ==========================================
#[derive(Debug, Deserialize)]
pub struct AccountData {
    pub details: Vec<BalanceDetail>,
}

#[derive(Debug, Deserialize)]
pub struct BalanceDetail {
    pub ccy: String,
    #[serde(rename = "availBal")]
    pub avail_bal: String,
    #[allow(dead_code)]
    #[serde(rename = "cashBal")]
    pub cash_bal: String,
}