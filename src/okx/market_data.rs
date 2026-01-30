use serde::{Deserialize, Deserializer, Serialize};

/// 📈 [Market Domain] Ticker 数据
/// 使用 f64 替代 String 以支持直接计算
/// 实现 Clone 以便存入 HashMap 状态中
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Ticker {
    #[serde(rename = "instId")]
    pub inst_id: String,

    // 🚀 性能优化点：使用自定义反序列化器，直接将 JSON 里的 String 解析为 f64
    #[serde(rename = "last", deserialize_with = "parse_f64_from_string")]
    pub last: f64,

    #[serde(rename = "vol24h", deserialize_with = "parse_f64_from_string")]
    pub volume: f64,

    #[serde(rename = "askPx", deserialize_with = "parse_f64_from_string")]
    pub ask_px: f64,

    #[serde(rename = "bidPx", deserialize_with = "parse_f64_from_string")]
    pub bid_px: f64,

    pub ts: String, // 时间戳保留字符串，避免精度问题，按需转换
}

/// 🛠️ [Helper] 自定义反序列化函数
/// 解决 OKX API 返回 {"last": "123.45"} 这种将数字包在字符串里的问题
/// 直接 parse 避免 String 内存分配
fn parse_f64_from_string<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    // 这里的 String 是临时的，serde 会尽量优化
    let s: String = Deserialize::deserialize(deserializer)?;
    s.parse::<f64>().map_err(serde::de::Error::custom)
}
