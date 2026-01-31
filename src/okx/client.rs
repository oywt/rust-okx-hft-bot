// src/okx/client.rs

pub(crate) use crate::okx::{auth, protocol::Endpoint};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::net::TcpStream;
use tokio_tungstenite::{client_async, WebSocketStream};
use url::Url;
use log::{info, error, warn};

use async_http_proxy::http_connect_tokio;
use native_tls::TlsConnector;
use tokio_native_tls::TlsConnector as TokioTlsConnector;
use tokio_tungstenite::tungstenite::Message;
type WsStream = WebSocketStream<tokio_native_tls::TlsStream<TcpStream>>;

pub struct OkxClient {
    endpoint: Endpoint,
}

impl OkxClient {
    pub fn new(endpoint: Endpoint) -> Self {
        OkxClient { endpoint }
    }

    pub async fn connect(&self, config: &crate::config::AppConfig) -> Option<WsStream> {
        let url_str = self.endpoint.as_url();
        let target_url = Url::parse(url_str).unwrap();
        let target_host = target_url.host_str().unwrap();
        let target_port = target_url.port_or_known_default().unwrap_or(443);

        // ==========================================
        // 🚦 智能分支：根据是否配置代理决定连接方式
        // ==========================================
        if let Some(proxy_url_str) = &config.proxy_url {
            // ➤ 分支 A: 走代理 (本地开发)
            let proxy_url = Url::parse(proxy_url_str).unwrap();
            let proxy_host = proxy_url.host_str().unwrap();
            let proxy_port = proxy_url.port().unwrap();

            info!("🔗 [模式] 代理连接: {} -> OKX", proxy_url_str);

            let mut tcp_stream = match TcpStream::connect(format!("{}:{}", proxy_host, proxy_port)).await {
                Ok(s) => s,
                Err(e) => { error!("❌ 代理连接失败: {}", e); return None; }
            };
            let _ = tcp_stream.set_nodelay(true);

            if let Err(e) = http_connect_tokio(&mut tcp_stream, target_host, target_port).await {
                error!("❌ 代理握手失败: {}", e); return None;
            }

            // 代理模式下，通常需要跳过证书验证 (防止MITM)
            let cx = TlsConnector::builder().danger_accept_invalid_certs(true).build().unwrap();
            let cx = TokioTlsConnector::from(cx);
            let tls_stream = match cx.connect(target_host, tcp_stream).await {
                Ok(s) => s, Err(e) => { error!("❌ TLS 失败: {}", e); return None; }
            };

            let (ws_stream, _) = client_async(url_str, tls_stream).await.ok()?;

            // 返回流
            match self.endpoint {
                Endpoint::Public => Some(ws_stream),
                Endpoint::Private => self.login(ws_stream, config).await,
            }

        } else {
            // ➤ 分支 B: 直连 (香港/东京服务器)
            info!("🔗 [模式] 直连 OKX (无代理) -> {}:{}", target_host, target_port);

            let tcp_stream = match TcpStream::connect(format!("{}:{}", target_host, target_port)).await {
                Ok(s) => s,
                Err(e) => { error!("❌ 直连失败 (请检查服务器网络): {}", e); return None; }
            };
            let _ = tcp_stream.set_nodelay(true);

            // 直连模式下，证书必须是合法的，不能跳过验证！
            let cx = TlsConnector::builder().build().unwrap();
            let cx = TokioTlsConnector::from(cx);
            let tls_stream = match cx.connect(target_host, tcp_stream).await {
                Ok(s) => s, Err(e) => { error!("❌ TLS 失败: {}", e); return None; }
            };

            let (ws_stream, _) = client_async(url_str, tls_stream).await.ok()?;

            // 返回流
            match self.endpoint {
                Endpoint::Public => Some(ws_stream),
                Endpoint::Private => self.login(ws_stream, config).await,
            }
        }
    }


    // ✨ [核心修改] 阻塞式登录：发包后等待响应，确认成功才返回
    async fn login(&self, ws_stream: WsStream, config: &crate::config::AppConfig) -> Option<WsStream> {
        let (mut write, mut read) = ws_stream.split(); // 注意这里 read 也是 mut
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

        // 1. 发送登录请求
        if let Err(e) = write.send(Message::Text(login_msg.to_string())).await {
            error!("❌ 登录包发送失败: {}", e);
            return None;
        }
        info!("📤 登录请求已发送，等待服务器确认...");

        // 2. ⏳ 原地等待响应 (关键！)
        // 我们只读第一条消息，它必须是登录结果
        while let Some(msg_res) = read.next().await {
            match msg_res {
                Ok(Message::Text(text)) => {
                    // 解析 JSON 检查 code
                    // 简易解析，只要包含 "login" 和 "0" 就认为成功
                    if text.contains("\"event\":\"login\"") && text.contains("\"code\":\"0\"") {
                        info!("✅ 登录鉴权成功 (Login Authorized)");
                        // 3. 登录成功，把流合并回去，交还给 main
                        return Some(write.reunite(read).unwrap());
                    } else if text.contains("\"event\":\"error\"") {
                        error!("❌ 登录被拒绝: {}", text);
                        return None;
                    } else {
                        warn!("⚠️ 收到非登录响应 (忽略): {}", text);
                    }
                },
                Ok(_) => {}, // 忽略 Ping/Pong 等其他帧
                Err(e) => {
                    error!("❌ 等待登录响应时断开: {}", e);
                    return None;
                }
            }
        }

        error!("❌ 连接在登录阶段意外关闭");
        None
    }
}
