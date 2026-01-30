use serde_json::json;
use tokio_tungstenite::tungstenite::Message;

/// 🌍 [业务领域] 连接端点类型
/// 区分公共数据通道和私有交易通道
#[derive(Debug, Clone, PartialEq)]
pub enum Endpoint {
    Public,  // 公共频道 (行情, K线) - 无需鉴权
    Private, // 私有频道 (交易, 账户) - 需要鉴权
}

impl Endpoint {
    /// 获取对应的 WebSocket URL
    pub fn as_url(&self) -> &'static str {
        match self {
            Endpoint::Public => "wss://ws.okx.com:8443/ws/v5/public",
            Endpoint::Private => "wss://ws.okx.com:8443/ws/v5/private",
        }
    }
}

/// 业务领域：定义我们支持的频道类型
pub enum ChannelType {
    Tickers,        // 行情频道
    // Orders,      // 订单频道 (属于 Private)
}

impl ChannelType {
    fn as_str(&self) -> &'static str {
        match self {
            ChannelType::Tickers => "tickers",
        }
    }
}

/// 🏭 [工厂方法] 生成订阅指令
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