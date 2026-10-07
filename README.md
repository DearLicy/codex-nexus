# Codex Nexus

Codex Nexus 是一个面向 Codex 工作流的本地优先第三方工具。它把日常任务、项目上下文和可扩展的辅助能力放在一个清晰的桌面界面中，同时把敏感数据留在用户自己的设备上。

> 项目仍在积极开发中。接口、存储格式和命令可能随版本调整。

## 主要方向

- 通过桌面界面查看和组织 Codex 工作上下文。
- 将本地状态、配置和可选的远程请求分开管理。
- 在不暴露凭据的前提下，为后续插件和自动化提供扩展边界。
- 在 macOS、Windows 和 Linux 上保持一致的核心行为（具体平台支持以 CI 和发布说明为准）。

## 快速开始

Codex Nexus 是 Tauri 桌面应用，启动后在独立的原生窗口中运行，默认界面语言为简体中文。需要 Node.js 20+、pnpm 9+；开发和打包桌面应用还需要 Rust stable 与 Tauri 的系统依赖。

安装依赖并启动桌面开发窗口：

```bash
pnpm install
pnpm dev:desktop
```

`pnpm dev` 只启动 Vite 前端预览，适合调试界面，不代表最终交付形态。真正的桌面开发入口是 `pnpm dev:desktop`，它会启动 Tauri 宿主、Rust 命令和本地能力。

构建可分发的桌面安装包：

```bash
pnpm build:desktop
```

打包结果位于 `src-tauri/target/release/bundle/`，具体格式由当前平台决定（macOS 通常为 `.app`/`.dmg`，Windows 通常为 `.msi`/`.exe`）。只构建前端静态资源时使用 `pnpm build`。

在提交前运行完整检查：

```bash
pnpm test
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
```

如果本地环境尚未安装 pnpm，可以使用 Corepack：

```bash
corepack enable
corepack prepare pnpm@latest --activate
```

## 仓库结构

```text
src/                 前端界面与共享类型
src-tauri/           Tauri 宿主、Rust 命令和本地能力
packages/gateway/    本地回环聚合网关
packages/image-mcp/  面向图片能力的 MCP stdio 服务
docs/                架构及维护文档
.github/workflows/   持续集成配置
```

## 隐私与安全

Codex Nexus 默认按本地优先方式工作。请把 API token、Cookie、私钥和生产数据放在未提交的本地配置或系统密钥链中，不要写入 issue、日志、截图或测试夹具。发现安全问题请按 [SECURITY.md](SECURITY.md) 的流程报告。

## 参与贡献

欢迎提交修复、文档改进和可复现的功能建议。请先阅读 [CONTRIBUTING.md](CONTRIBUTING.md)，并在提交前运行项目检查。

## 许可证

本项目以 MIT License 发布，详见 [LICENSE](LICENSE)。第三方组件的许可信息见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。

---

## English overview

Codex Nexus is a local-first third-party companion for Codex workflows. It brings task context, project information, and extensible helpers into a focused desktop interface while keeping sensitive data on the user’s device by default.

> The project is under active development. APIs, storage formats, and commands may change between releases.

### Getting started

Codex Nexus is a Tauri desktop application. Its default UI language is Simplified Chinese and it runs in a native application window. Install Node.js 20+, pnpm 9+, Rust stable, and the platform dependencies required by Tauri:

```bash
pnpm install
pnpm dev:desktop
```

`pnpm dev` starts only the Vite browser preview for frontend work. Use `pnpm dev:desktop` for the actual desktop app and `pnpm build:desktop` to produce platform installers under `src-tauri/target/release/bundle/`.

Run the checks used before a pull request:

```bash
pnpm test
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
```

### Layout

- `src/` contains the frontend and shared types.
- `src-tauri/` contains the Tauri host, Rust commands, and local integrations.
- `packages/gateway/` contains the local loopback aggregation gateway.
- `packages/image-mcp/` contains the image-focused MCP stdio service.
- `docs/` contains architecture and maintenance notes.
- `.github/workflows/` contains CI configuration.

### Privacy and security

Codex Nexus is local-first by default. Keep API tokens, cookies, private keys, and production data in local configuration or the system keychain. Do not commit them or place them in issues, logs, screenshots, or fixtures. See [SECURITY.md](SECURITY.md) for vulnerability reporting.

### Contributing and license

Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request. The project is available under the MIT License; see [LICENSE](LICENSE). Third-party notices are collected in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
