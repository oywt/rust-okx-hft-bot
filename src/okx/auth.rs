use hmac::{Hmac, Mac};
use sha2::Sha256;
use base64::{Engine as _, engine::general_purpose};

// 定义 HMAC-SHA256 类型别名
type HmacSha256 = Hmac<Sha256>;

/// 生成 OKX WebSocket 鉴权签名
/// 公式: Base64(HmacSHA256(timestamp + "GET" + "/users/self/verify", secret_key))
pub fn generate_sign(secret: &str, timestamp: &str) -> String {
    let method = "GET";
    let request_path = "/users/self/verify";
    let body = ""; // 登录消息体为空

    // 1. 拼接签名源字符串
    let message = format!("{}{}{}{}", timestamp, method, request_path, body);

    // debug!("📝 [鉴权] 签名源字符串: {}", message);

    // 2. 初始化 HMAC 计算器
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC 初始化失败: Key 长度无效");

    // 3. 注入数据
    mac.update(message.as_bytes());

    // 4. 计算并 Base64 编码
    let result = mac.finalize();
    let sign = general_purpose::STANDARD.encode(result.into_bytes());

    sign
}