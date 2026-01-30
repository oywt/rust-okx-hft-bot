use crate::okx::{auth, protocol::Endpoint}; // 引入 Endpoint
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::net::TcpStream;
use tokio_tungstenite::{client_async, WebSocketStream};
use url::Url;
use log::{info, error, warn};

// 引入代理和 TLS 库
use async_http_proxy::http_connect_tokio;
use native_tls::TlsConnector;
use tokio_native_tls::TlsConnector as TokioTlsConnector;

type WsStream = WebSocketStream<tokio_native_tls::TlsStream<TcpStream>>;

pub struct OkxClient {
    endpoint: Endpoint, // [新增] 客户端持有当前的业务领域
}

impl OkxClient {
    /// 初始化时指定业务领域 (Public 或 Private)
    pub fn new(endpoint: Endpoint) -> Self {
        OkxClient { endpoint }
    }

    pub async fn connect(&self, config: &crate::config::AppConfig) -> Option<WsStream> {
        let url_str = self.endpoint.as_url();
        info!("🔌 [HFT] 正在连接业务端点: {:?} ({})", self.endpoint, url_str);

        let target_url = Url::parse(url_str).unwrap();
        let target_host = target_url.host_str().unwrap();
        let target_port = 8443;

        // 1. 代理处理 (保持不变)
        let proxy_url_str = config.proxy_url.as_ref().expect("❌ 未配置 PROXY_URL");
        let proxy_url = Url::parse(proxy_url_str).unwrap();
        let proxy_host = proxy_url.host_str().unwrap();
        let proxy_port = proxy_url.port().unwrap();

        let mut tcp_stream = match TcpStream::connect(format!("{}:{}", proxy_host, proxy_port)).await {
            Ok(s) => s,
            Err(e) => { error!("❌ 代理连接失败: {}", e); return None; }
        };

        if let Err(e) = http_connect_tokio(&mut tcp_stream, target_host, target_port).await {
            error!("❌ 代理握手失败: {}", e);
            return None;
        }

        // 2. TLS 加密 (保持不变)
        let cx = TlsConnector::builder().build().unwrap();
        let cx = TokioTlsConnector::from(cx);
        let tls_stream = match cx.connect(target_host, tcp_stream).await {
            Ok(s) => s,
            Err(e) => { error!("❌ TLS 握手失败: {}", e); return None; }
        };

        // 3. WebSocket 握手
        let (ws_stream, _) = match client_async(url_str, tls_stream).await {
            Ok(v) => v,
            Err(e) => { error!("❌ WS 升级失败: {}", e); return None; }
        };

        info!("✅ WebSocket 通道建立成功！");

        // ======================================================
        // 🧠 [领域逻辑] 根据 Endpoint 类型决定是否鉴权
        // ======================================================
        match self.endpoint {
            Endpoint::Public => {
                info!("🌍 [Public] 公共频道无需登录，连接就绪。");
                Some(ws_stream) // 直接返回，不登录
            },
            Endpoint::Private => {
                info!("🔐 [Private] 私有频道需要鉴权，正在执行登录...");
                self.login(ws_stream, config).await
            }
        }
    }

    /// 私有方法：专门处理登录逻辑
    async fn login(&self, ws_stream: WsStream, config: &crate::config::AppConfig) -> Option<WsStream> {
        let (mut write, mut read) = ws_stream.split();
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let sign = auth::generate_sign(&config.okx_secret_key, &timestamp);

        let login_msg = json!({
            "op": "login",
            "args": [{
                "apiKey": config.okx_api_key,
                "passphrase": config.okx_passphrase,
                "timestamp": timestamp,
                "sign": sign
            }]
        });

        if let Err(e) = write.send(tokio_tungstenite::tungstenite::Message::Text(login_msg.to_string())).await {
            error!("❌ 发送登录包失败: {}", e);
            return None;
        }

        info!("📤 登录请求已发送，返回合并流...");
        // 注意：这里我们简化处理，假设发出去就算成功，实际应该等回包。
        // 为了架构解耦，我们把流还给 Main，让 Main 的 Strategy 去读登录回执。
        Some(write.reunite(read).unwrap())
    }
}