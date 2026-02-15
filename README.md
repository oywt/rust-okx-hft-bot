

```markdown
# 🦀 Rust HFT Bitcoin Trading Bot (High-Frequency Trading)

基于 Rust (Tokio) 构建的高性能加密货币高频交易机器人，对接 OKX V5 API。
采用领域驱动设计 (DDD) 架构，实现了 WebSocket 代理隧道、TLS 加密连接及自动鉴权。

## 🚀 核心特性 (Key Features)

* **极速网络层**: 基于 `tokio-tungstenite` 和 `native-tls` 实现低延迟 WebSocket 连接。
* **安全隧道**: 内置 HTTP Proxy + TLS 握手，支持在复杂网络环境下（如防火墙后）穿透连接。
* **领域驱动设计 (DDD)**: 
    * `Public Domain`: 负责高频行情订阅 (Market Data)。
    * `Private Domain`: 负责交易指令下发与账户风控 (Order Execution)。
* **高鲁棒性**: 完善的错误处理与自动重连机制（开发中）。
* **策略引擎**: 实现了 AHR999 囤币指标计算与动态定投策略。

## 🛠️ 技术栈 (Tech Stack)

* **Language**: Rust (2021 Edition)
* **Async Runtime**: Tokio
* **Network**: Tungstenite (WebSocket), Async-Http-Proxy, Native-TLS
* **Serialization**: Serde, Serde-JSON
* **Logging**: Env_Logger

## 📦 快速开始 (Quick Start)

1. **克隆仓库**
   ```bash
   git clone [https://github.com/YourName/rust-hft-bot.git](https://github.com/YourName/rust-hft-bot.git)

```

2. **配置环境变量**
   复制 `.env.example` 为 `.env` 并填入你的 OKX API Key：
```env
OKX_API_KEY=your_api_key
OKX_SECRET_KEY=your_secret_key
OKX_PASSPHRASE=your_passphrase
PROXY_URL=[http://127.0.0.1:7890](http://127.0.0.1:7890)

```


3. **运行**
```bash
cargo run --release

```



## ⚠️ 免责声明

本项目仅供学习 Rust 高并发编程与量化架构设计使用，实盘交易请自行承担风险。

```

#### 第二步：创建 `.env.example`
```bash
# OKX API Configuration
OKX_API_KEY=
OKX_SECRET_KEY=
OKX_PASSPHRASE=

# Network Proxy (Required for restricted regions)
PROXY_URL=http://127.0.0.1:7890

# Trading Config
SIMULATION_MODE=true

```

#### 第三步：推送到 GitHub

在你的终端里（项目根目录）执行：

```bash
git init
git add .
git commit -m "feat: init project with Rust HFT architecture"
git branch -M main
# 去 GitHub 创建一个新仓库，然后把下面这行换成你的仓库地址
git remote add origin https://github.com/你的用户名/rust-hft-bot.git
git push -u origin main

```

---

