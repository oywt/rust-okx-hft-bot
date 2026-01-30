use chrono::Utc;

/// 获取当前 UTC 时间戳 (秒级) - 用于 OKX 鉴权
pub fn get_timestamp_sec() -> String {
    Utc::now().timestamp().to_string()
}

/// 获取当前 UTC 时间戳 (毫秒级) - 用于计算延迟
pub fn get_timestamp_ms() -> i64 {
    Utc::now().timestamp_millis()
}