# RustTide

使用 **Rust + Tauri** 重写的 macOS 翻译工具，灵感来自
[TypeTide](https://github.com/everettjf/TypeTide)。采用 **MIT License**。

> 项目处于初始化与开发阶段，首版功能尚未完成，暂未发布安装包。

## 首版范围

- macOS 选中文字取词，支持辅助功能 API 与剪贴板取词。
- 调用 OpenAI 兼容 API 翻译，支持自定义地址、模型和目标语言。
- 译文预览、复制，以及替换回原输入框。
- 快捷键、API 设置、权限状态与菜单栏入口。
- API 密钥保存在 macOS 钥匙串中。

首版暂不包含离线模型与 Windows 版本。

## 技术结构

- Rust：翻译请求、设置、操作状态与桌面命令。
- Tauri 2：桌面外壳和本地 WebView。
- HTML/CSS/JavaScript：翻译预览和设置界面。
- Objective-C：薄层 macOS Accessibility、AppKit 与 Keychain 桥接。

## 开发路线

1. 初始化仓库、MIT 协议与 GitHub Desktop。
2. 完成 macOS 取词、翻译、替换和设置。
3. 验证 API、取消操作、焦点变化与剪贴板恢复。
4. 发布 macOS 首版。

详细范围见 [实施计划](docs/implementation-plan.md)。

## 许可证与致谢

本项目使用 [MIT License](LICENSE)。感谢
[everettjf/TypeTide](https://github.com/everettjf/TypeTide) 提供参考。
