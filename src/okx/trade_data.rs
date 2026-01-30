use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[allow(dead_code)] // 暂时抑制警告
pub struct Order {
    #[serde(rename = "instId")]
    pub inst_id: String,
    #[serde(rename = "ordId")]
    pub ord_id: String,
    #[serde(rename = "clOrdId")]
    pub client_oid: String,
    pub px: String,
    pub sz: String,
    pub side: String,
    pub state: String,
    #[serde(rename = "cTime")]
    pub create_time: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[allow(dead_code)] // 暂时抑制警告
pub struct Balance {
    #[serde(rename = "ccy")]
    pub currency: String,
    #[serde(rename = "cashBal")]
    pub cash_balance: String,
}
