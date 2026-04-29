# API Switch - Rust 版本

> 高性能 AI API 管理与转发中心

基于 [wang1970/API-Switch](https://github.com/wang1970/API-Switch) 的 Rust 重写版本，专为高并发场景优化。

## ✨ 特性

| 特性 | 说明 |
|------|------|
| **高性能** | Rust + Axum，镜像仅 38MB（比 Node.js 版小 83%） |
| **多渠道路由** | 一个入口访问多个 AI 服务商，自动故障转移 |
| **智能熔断** | 失败自动冷却，成功自动恢复 |
| **Token 计数** | 精确统计 prompt_tokens 和 completion_tokens |
| **模型自动发现** | 一键拉取渠道可用模型 |
| **API Pool** | 条目管理 + 拖拽排序 + 分组显示 |
| **OpenAI 兼容** | 完整兼容 `/v1/chat/completions` 和 `/v1/models` |

## 🚀 快速开始

### Docker 部署

```bash
# 构建镜像
docker build -t api-switch:rust .

# 运行容器
docker run -d \
  --name api-switch \
  --restart unless-stopped \
  --network host \
  -v /data/api-switch:/app/data \
  api-switch:rust
```

### 访问 Web UI

```
URL: http://your-ip:9091
默认账户: root / admin
```

## 📦 API 端点

### 认证
- `POST /api/login` — 登录
- `POST /api/change-password` — 修改密码

### 渠道管理
- `GET /api/channels` — 列表
- `POST /api/channels` — 创建
- `PUT /api/channels/:id` — 更新
- `DELETE /api/channels/:id` — 删除
- `POST /api/channels/:id/discover` — 发现模型
- `POST /api/channels/:id/test` — 测试延迟

### API Pool (Entries)
- `GET /api/entries` — 列表
- `POST /api/entries` — 创建
- `PUT /api/entries/:id` — 更新
- `DELETE /api/entries/:id` — 删除
- `POST /api/entries/reorder` — 排序
- `POST /api/entries/:id/toggle` — 启用/禁用

### API Key
- `GET /api/keys` — 列表
- `POST /api/keys` — 创建
- `DELETE /api/keys/:id` — 删除

### Dashboard
- `GET /api/dashboard/stats` — 统计
- `GET /api/dashboard/models` — 模型排名

### 代理
- `POST /v1/chat/completions` — Chat API
- `GET /v1/models` — 模型列表

## 🔧 配置

### 熔断器

| 参数 | 默认值 | 说明 |
|------|--------|------|
| failure_threshold | 5 | 失败次数阈值 |
| recovery_secs | 300 | 恢复时间（秒） |
| retry_times | 3 | 重试次数 |
| timeout | 60000 | 请求超时（毫秒） |

### 环境变量

无需配置环境变量，数据存储在 `/app/data/api-switch.db`

## 📊 技术栈

| 层级 | 技术 |
|------|------|
| 后端 | Rust nightly + Axum 0.7 |
| 数据库 | SQLite (rusqlite bundled) |
| HTTP 客户端 | reqwest |
| 前端 | 原生 JS + Tailwind CSS |
| 容器 | Alpine 3.19 |

## 📝 开发

```bash
# 本地编译
cargo build --release

# 运行
./target/release/api-switch
```

## 📜 License

MIT License

## 🙏 致谢

- 原版 [API-Switch](https://github.com/wang1970/API-Switch) by wang1970
- 灵感来自 [New API](https://github.com/QuantumNous/new-api)
