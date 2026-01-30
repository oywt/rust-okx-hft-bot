pub mod auth;
pub mod client;

pub mod protocol;

// ✅ [DDD] 领域驱动：分离行情与交易数据
pub mod market_data; // 新增
pub mod trade_data;  // 新增