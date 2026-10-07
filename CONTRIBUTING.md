# Contributing to Codex Nexus

感谢你考虑为 Codex Nexus 贡献代码。小而聚焦的变更最容易审查；行为变化请同时更新文档或测试。

## 开始开发

1. Fork 或克隆仓库，并安装 Node.js 20+、pnpm 9+ 与 Rust stable。
2. 运行 `pnpm install` 安装依赖。
3. 使用 `pnpm dev:desktop` 启动 Tauri 桌面开发窗口；只有在专门调试前端时才使用 `pnpm dev` 的 Vite 浏览器预览。
4. 在提交前运行 `pnpm test` 和 `pnpm build`；若修改 Rust 代码，也运行 `cargo fmt --all -- --check` 与 `cargo check --manifest-path src-tauri/Cargo.toml`。

## 提交变更

- 每个提交只解决一个问题，避免把无关的格式化混入功能改动。
- 新增用户可见行为时，更新 README 或 `docs/` 中对应说明。
- 不要提交凭据、个人数据、构建产物、`node_modules` 或本机 IDE 配置。
- 对外部输入、文件路径和网络响应进行校验；错误信息应帮助用户修复问题，但不能泄露秘密。
- 保持前端与 Tauri 命令的类型和错误语义一致。

## Pull request

PR 描述请说明：问题、实现方式、验证命令，以及任何兼容性或迁移影响。请确保 CI 通过，并在涉及界面时附上截图或短视频（注意遮盖敏感信息）。维护者可能要求拆分过大的 PR 或补充测试。

## 代码风格

遵循仓库现有的 formatter、linter 和 TypeScript/Rust 编译器设置。优先使用已有依赖和抽象；引入新依赖时，请说明必要性、许可证和包体积影响，并更新 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)（若该依赖带来新的声明要求）。

## 许可证

提交代码即表示你有权提交该代码，并同意按仓库 [MIT License](LICENSE) 授权。第三方代码必须保留其原始许可和版权声明。
