use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite::Message;
use serde_json::json;
use serde_json::value::RawValue;

/// 🌍 [业务领域] 连接端点类型
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)] // 暂时允许未使用（如果main只用了Public）
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

/// 业务领域：定义频道
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // 暂时允许 Orders/Account 未被使用
pub enum ChannelType {
    Tickers,
    Orders,
    Account,
}

impl ChannelType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChannelType::Tickers => "tickers",
            ChannelType::Orders => "orders",
            ChannelType::Account => "account",
        }
    }
}


#[derive(Debug, Deserialize)]
pub struct WsRouter<'a> {
    #[serde(borrow)]
    pub arg: Option<Arg<'a>>,
    #[serde(borrow)]
    pub data: Option<&'a RawValue>,
    pub event: Option<&'a str>,
    pub code: Option<&'a str>,
    pub msg: Option<&'a str>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Arg<'a> {
    pub channel: &'a str,
    #[serde(rename = "instId")]
    pub inst_id: &'a str,
}

pub fn create_subscribe_packet(channel: ChannelType, inst_id: &str) -> Message {
    let payload = json!({
        "op": "subscribe",
        "args": [{
            "channel": channel.as_str(),
            "instId": inst_id
        }]
    });
    Message::Text(payload.to_string())
}
