# Codex Nexus Native

这是 Codex Nexus 的 Slint 原生桌面入口。它不创建 WebView，也不依赖 React、Vite 或浏览器路由；界面由 Slint 编译为原生渲染代码，数据直接从 Rust 的 `codex_tool_core::discovery` 读取。

在安装 Rust stable 和系统图形依赖后运行：

```bash
cargo run --manifest-path native/Cargo.toml
```

窗口启动时会扫描 `CODEX_HOME/sessions`、`generated_images` 和 `archived_sessions`，显示本机会话数量、受管文件数量、占用大小和前几条实际记录。点击右上角“刷新”会重新扫描，不使用演示数组。

这是主应用迁移到原生桌面渲染的第一步；后续页面继续复用同一组 Rust 命令和数据模型。
