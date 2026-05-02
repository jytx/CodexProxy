# CodexProxy

将 Codex CLI（OpenAI Responses API）的请求代理转发到国内模型厂商（如 GLM、MiniMax 等）的 Chat Completions API，自动完成协议转换，让 Codex 可以直接使用国内大模型。

## 功能特性

- **协议转换**：自动将 Responses API 请求转为 Chat Completions API 格式
- **多配置管理**：支持创建多个代理配置，拖拽排序，一键切换
- **模型覆盖**：可配置模型名称覆盖，替换 Codex 请求中的模型名
- **请求日志**：实时显示代理请求，支持 JSON 折叠查看、下载、复制
- **日志持久化**：请求日志自动写入 JSONL 文件，重启应用可加载历史
- **主题切换**：亮色 / 暗色模式
- **工具透传**：支持 function、computer_use 等工具类型透传

## 技术栈

- **前端**：React 19 + TypeScript + Vite
- **后端**：Rust + Tauri v2
- **HTTP 服务**：axum（代理服务器）
- **JSON 查看**：react18-json-view

## 项目结构

```
src/                        # 前端 React 代码
  components/
    TopBar.tsx              # 标题栏（主题切换、日志入口）
    ProxyPanel.tsx          # 代理管理面板
    ProxyCard.tsx           # 代理配置卡片（支持拖拽排序）
    ProxyModal.tsx          # 代理配置编辑弹窗
    LogPanel.tsx            # 请求日志面板
    JsonViewer.tsx          # JSON 折叠查看器
  hooks/
    useProxyStore.ts        # 代理状态管理
  styles/                   # CSS 样式
  types/
    app.ts                  # TypeScript 类型定义

src-tauri/src/              # 后端 Rust 代码
  main.rs                   # 应用入口
  lib.rs                    # Tauri 命令注册
  proxy.rs                  # HTTP 代理服务器（axum）
  convert.rs                # Responses API ↔ Chat Completions API 协议转换
  stream.rs                 # SSE 流式处理
  log_writer.rs             # JSONL 日志持久化
  codex_config.rs           # Codex 配置读写
  types.rs                  # Rust 类型定义
```

## 快速开始

### 环境要求

- Node.js >= 18
- Rust >= 1.77
- macOS（已测试）

### 安装依赖

```bash
# 安装前端依赖
npm install

# Rust 依赖会在首次构建时自动安装
```

### 开发模式

```bash
./scripts/start.sh
```

### 构建发布

```bash
cd src-tauri && cargo tauri build
```

## 使用方式

1. 启动应用后，点击「添加配置」创建代理
2. 填写上游 API 地址（如 `https://open.bigmodel.cn/api/paas/v4`）和 API Key
3. 可选填写模型名称覆盖（如 `glm-5.1`）
4. 点击启动，代理会在本地监听指定端口
5. 将 Codex 的 API 地址指向 `http://localhost:{port}`

### Codex 配置示例

编辑 `~/.codex/config.toml`：

```toml
[api]
base_url = "http://localhost:9001/v1"
api_key = "your-api-key"
model = "codex-proxy"
```

## 支持的路由

| 路由 | 说明 |
|------|------|
| `/v1/responses` | Responses API（协议转换 → Chat Completions） |
| `/v1/chat/completions` | Chat Completions API（直接透传） |
| `/responses` | 同上（不带 `/v1` 前缀） |
| `/chat/completions` | 同上（不带 `/v1` 前缀） |
| `/api/coding/paas/v4/responses` | 部分厂商专用路径 |
| `/health` | 健康检查 |
