use colored::*;
use chrono::Utc;

use crate::okx::market_data::Ticker;

pub struct LogFormatter;

impl LogFormatter {
    /// 🎨 [组件功能] 格式化 Ticker 日志
    /// 优势：直接接受 Ticker 引用，内聚性更强，参数更简洁
    pub fn format_ticker(ticker: &Ticker) -> String {
        // 1. 业务计算 (Spread)
        // 注意：这里只做展示用的计算，不涉及核心策略逻辑
        let spread = ticker.ask_px - ticker.bid_px;

        // 2. ⏱️ 延迟计算
        // 解析 OKX 时间戳 (如果解析失败默认为 0)
        let remote_ts = ticker.ts.parse::<i64>().unwrap_or(0);
        let local_ts = Utc::now().timestamp_millis();
        let latency = local_ts - remote_ts;

        // 3. 🎨 动态颜色判断
        let latency_display = if latency < 100 {
            format!("{}ms", latency).green()
        } else if latency < 300 {
            format!("{}ms", latency).yellow()
        } else {
            format!("{}ms", latency).red()
        };

        // 4. 组装日志
        format!(
            "⚡ [{}] Last: {} | Spread: {:.2} | Latency: {}",
            ticker.inst_id.cyan().bold(),
            ticker.last.to_string().yellow(),
            spread,
            latency_display
        )
    }
}
