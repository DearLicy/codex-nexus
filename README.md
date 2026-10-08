# Codex Nexus

Codex Nexus 是一个面向 Codex 工作流的本地优先第三方工具。它把日常任务、项目上下文和可扩展的辅助能力放在一个清晰的桌面界面中，同时把敏感数据留在用户自己的设备上。

> 项目仍在积极开发中。接口、存储格式和命令可能随版本调整。

## 主要方向

- 通过桌面界面查看和组织 Codex 工作上下文。
- 将本地状态、配置和可选的远程请求分开管理。
- 在不暴露凭据的前提下，为后续插件和自动化提供扩展边界。
- 在 macOS、Windows 和 Linux 上保持一致的核心行为（具体平台支持以 CI 和发布说明为准）。

## 快速开始

Codex Nexus 是由 Rust 和 Slint 编译的原生桌面应用。主程序不包含 WebView、HTML 页面、React 或浏览器路由，默认界面语言为简体中文。桌面应用只需要 Rust stable 和当前平台的图形工具链；Node.js/pnpm 仅用于本地聚合网关和图片 MCP 的 JavaScript 服务。

启动原生桌面窗口：

```bash
cargo run --manifest-path native/Cargo.toml
```

等价的 pnpm 命令是 `pnpm dev`。窗口中的会话、文件数量和占用空间直接从 Rust 核心扫描 `CODEX_HOME`，不会使用演示数组。

构建可分发的桌面安装包：

```bash
cargo build --release --manifest-path native/Cargo.toml
```

原生可执行文件位于 `native/target/release/codex-nexus-native`（Windows 为 `.exe`）。应用不再生成 `dist/`、Tauri bundle 或 HTML 预览；macOS `.app`、DMG 和 Windows 安装包由发布工作流使用各平台原生打包工具生成。

macOS 本地打包使用 `pnpm package:macos`，会创建 `.app` 并在 macOS 上生成 DMG；设置 `CODEX_NEXUS_SIGNING_IDENTITY` 后会额外执行签名。Windows 本地打包使用 `pnpm package:windows`，会创建包含 `.exe` 和 README 的 ZIP。正式发布仍需在目标平台配置签名、 notarization 或 Windows 代码签名。

在提交前运行完整检查：

```bash
pnpm test
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path native/Cargo.toml
```

如果本地环境尚未安装 Rust，可以使用 rustup 安装 stable 工具链。需要运行网关或图片 MCP 时，如果本地环境尚未安装 pnpm，可以使用 Corepack：

```bash
corepack enable
corepack prepare pnpm@latest --activate
```

## 仓库结构

```text
native/              Slint 原生桌面应用、UI 和平台打包入口
src-tauri/           可复用 Rust 核心、扫描器、路由和安全存储
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

Codex Nexus is a native desktop application compiled from Rust and Slint. The main binary has no WebView, HTML page, React shell, or browser router, and its default UI language is Simplified Chinese. Install Rust stable and the platform graphics toolchain. Node.js/pnpm is only needed for the optional gateway and image MCP services:

```bash
cargo run --manifest-path native/Cargo.toml
```

`pnpm dev` is an alias for the same native command. Use `cargo build --release --manifest-path native/Cargo.toml` to create the release binary. Platform installers are produced by the release workflow with native packaging tools.

Run the checks used before a pull request:

```bash
pnpm test
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path native/Cargo.toml
```

### Layout

- `native/` contains the Slint native desktop application and UI.
- `src-tauri/` contains the reusable Rust core, scanners, routing, and secure storage.
- `packages/gateway/` contains the local loopback aggregation gateway.
- `packages/image-mcp/` contains the image-focused MCP stdio service.
- `docs/` contains architecture and maintenance notes.
- `.github/workflows/` contains CI configuration.

### Privacy and security

Codex Nexus is local-first by default. Keep API tokens, cookies, private keys, and production data in local configuration or the system keychain. Do not commit them or place them in issues, logs, screenshots, or fixtures. See [SECURITY.md](SECURITY.md) for vulnerability reporting.

### Contributing and license

Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request. The project is available under the MIT License; see [LICENSE](LICENSE). Third-party notices are collected in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
