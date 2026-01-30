// src/okx/client.rs

use crate::okx::{auth, protocol::Endpoint};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::net::TcpStream;
use tokio_tungstenite::{client_async, WebSocketStream};
use url::Url;
use log::{info, error};

use async_http_proxy::http_connect_tokio;
use native_tls::TlsConnector;
use tokio_native_tls::TlsConnector as TokioTlsConnector;

type WsStream = WebSocketStream<tokio_native_tls::TlsStream<TcpStream>>;

pub struct OkxClient {
    endpoint: Endpoint,
}

impl OkxClient {
    pub fn new(endpoint: Endpoint) -> Self {
        OkxClient { endpoint }
    }

    pub async fn connect(&self, config: &crate::config::AppConfig) -> Option<WsStream> {
        // 1. 解析目标 URL
        let url_str = self.endpoint.as_url();
        let target_url = Url::parse(url_str).unwrap();
        let target_host = target_url.host_str().unwrap();
        // 自动识别端口：如果是 wss:// 则默认为 443
        let target_port = target_url.port_or_known_default().unwrap_or(443);

        // 2. 解析代理配置
        let proxy_url_str = config.proxy_url.as_ref().expect("❌ 未配置 PROXY_URL");
        let proxy_url = Url::parse(proxy_url_str).unwrap();
        let proxy_host = proxy_url.host_str().unwrap();
        let proxy_port = proxy_url.port().unwrap();

        info!("🔗 连接路径: 本地 -> 代理({}:{}) -> OKX({}:{})",
            proxy_host, proxy_port, target_host, target_port);

        // 3. TCP 连接代理
        let mut tcp_stream = match TcpStream::connect(format!("{}:{}", proxy_host, proxy_port)).await {
            Ok(s) => s,
            Err(e) => {
                error!("❌ 连接代理服务器失败: {}", e);
                error!("👉 请检查: 1. v2rayN 是否启动 2. .env端口是否填对(默认10809?)");
                return None;
            }
        };

        // 优化：禁用 Nagle 算法
        let _ = tcp_stream.set_nodelay(true);

        // 4. HTTP 隧道握手
        if let Err(e) = http_connect_tokio(&mut tcp_stream, target_host, target_port).await {
            error!("❌ 代理隧道握手失败 (EOF通常意味着端口协议不对，比如连到了Socks端口): {}", e);
            return None;
        }

        // 5. TLS 握手
        // 🛡️ 容错模式：允许无效证书（防止代理软件MITM干扰）
        let cx = TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();
        let cx = TokioTlsConnector::from(cx);

        let tls_stream = match cx.connect(target_host, tcp_stream).await {
            Ok(s) => s,
            Err(e) => { error!("❌ TLS 握手失败: {}", e); return None; }
        };

        // 6. WebSocket 升级
        let (ws_stream, _) = match client_async(url_str, tls_stream).await {
            Ok(v) => v,
            Err(e) => { error!("❌ WS 升级失败: {}", e); return None; }
        };

        info!("✅ OKX WebSocket 连接建立成功！");

        match self.endpoint {
            Endpoint::Public => Some(ws_stream),
            Endpoint::Private => self.login(ws_stream, config).await,
        }
    }

    async fn login(&self, ws_stream: WsStream, config: &crate::config::AppConfig) -> Option<WsStream> {
        let (mut write, read) = ws_stream.split();
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
            error!("❌ 登录包发送失败: {}", e);
            return None;
        }

        info!("📤 登录请求已发送");
        Some(write.reunite(read).unwrap())
    }
}
