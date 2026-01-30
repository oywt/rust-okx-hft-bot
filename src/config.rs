use dotenv::dotenv;
use log::info;
use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub okx_api_key: String,
    pub okx_secret_key: String,
    pub okx_passphrase: String,
    pub simulation_mode: bool,
    
    pub proxy_url: Option<String>,
}

impl AppConfig {
    pub fn load() -> Self {
        dotenv().ok();
        info!("⚙️ [系统] 正在加载环境配置...");

        let api_key = env::var("OKX_API_KEY").expect("❌ 缺少 OKX_API_KEY");
        let secret_key = env::var("OKX_SECRET_KEY").expect("❌ 缺少 OKX_SECRET_KEY");
        let passphrase = env::var("OKX_PASSPHRASE").expect("❌ 缺少 OKX_PASSPHRASE");

        let sim_mode = env::var("SIMULATION_MODE")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .unwrap_or(true);

        // [新增] 读取代理配置 (允许为空，万一以后你在国外跑就不需要了)
        let proxy = env::var("PROXY_URL").ok();
        if let Some(ref p) = proxy {
            info!("🌐 [网络] 已启用代理服务: {}", p);
        }

        AppConfig {
            okx_api_key: api_key,
            okx_secret_key: secret_key,
            okx_passphrase: passphrase,
            simulation_mode: sim_mode,
            proxy_url: proxy, // 赋值
        }
    }
}