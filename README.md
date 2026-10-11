# RustTide

在当前应用里输入，按快捷键，原地译成另一种语言。

RustTide 是使用 **Rust + Tauri** 实现的 macOS 菜单栏翻译助手，交互参考
[TypeTide](https://github.com/everettjf/typetide)。采用 **MIT License**。

> v0.2.0：菜单栏常驻、输入框原地改写、选区旁流式浮窗。
> 发行包目前使用本地临时签名，尚未经过 Apple Developer ID 签名与公证。

## 怎么用

首次启动填写 API 配置，并在 **系统设置 → 隐私与安全性 → 辅助功能**
中允许 RustTide。保存配置，测试连接，关闭设置即可开始使用。

| 操作 | 默认快捷键 | 效果 |
| --- | --- | --- |
| 写入 | **Option+R** | 在当前输入框中翻译并直接替换；有选区时只改选区，无选区时改整个可编辑输入框 |
| 读取 | **Option+D** | 读取选中文字，在文字附近显示小浮窗；可复制，或替换可写选区 |
| 撤销 | **Command+Z** | 使用原应用的撤销功能 |
| 取消 | 浮窗“取消” / Escape / 菜单栏“取消当前翻译” | 中止尚未完成的请求 |

不需要先打开 RustTide 的窗口或把文字粘贴进去。浮窗出现时保留原应用的
输入焦点；写入完成后自动收起。设置从菜单栏打开，关闭设置后快捷键继续
运行；结束运行请使用菜单栏“退出 RustTide”。

## 可配置项

- **母语与外语**：默认简体中文与英语。
- **快捷键方向**：智能互译、固定译为母语、固定译为外语。
  智能方向识别原文；第三种语言或识别不确定时，读取译为母语，写入译为外语。
- **完成方式**：每个快捷键可选择小浮窗预览或直接替换。
  写入预览仍支持无选区的整个输入框。
- **API**：自定义 OpenAI 兼容地址、模型、超时、系统 HTTPS 代理。
- **表达风格**：自然、正式、简洁。
- **排除应用**：按 Bundle ID 禁用指定应用中的取词。
- **剪贴板取词**：辅助功能无法读取选区时，通过复制读取并恢复剪贴板。

选区浮窗优先使用文字的屏幕位置；应用不提供位置时退回输入框或鼠标位置。
只读内容可复制译文。翻译响应不完整、原文/选区改变、切换到其他应用时，
停止替换；完整译文在替换失败后仍可复制。不要在等待替换时继续修改原文。

API 支持 `/chat/completions` 流式响应；兼容服务返回普通 JSON 时，也可直接
使用同一响应。远程服务必须使用 HTTPS，本机回环地址可使用 HTTP。API
测试只发送固定示例“你好，世界！”。密钥按 API 地址分别保存在 macOS
钥匙串，设置文件和前端不会读回密钥。

## 技术结构

- Rust：语言方向、API 流式请求、设置、取消与操作状态。
- Tauri 2：菜单栏、全局快捷键和设置窗口。
- HTML/CSS/JavaScript：设置界面。
- Objective-C：macOS Accessibility、Keychain、语言识别与不抢焦点的 NSPanel。

本阶段专注 macOS 与 API；离线模型、Windows、自动划词触发尚未实现。
原文仅在主动触发时发送到配置的 API，不保存翻译历史。

## 本地开发

需要 macOS 12 或更高版本、Xcode Command Line Tools、Rust 1.90 或更新版本、
Node.js 与 npm。开发机为 Apple Silicon Mac。

```sh
npm ci
npm run dev
```

```sh
npm test
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
npm run build
```

应用产物为 `src-tauri/target/release/bundle/macos/RustTide.app`。
使用系统 WebView，无需独立浏览器运行时或本地 HTTP 服务。

## 兼容性与隐私

不同应用提供的辅助功能信息不同。无法读取当前输入框时，可在原应用中
选中文字后再次触发；无法确认目标的内容时停止替换。密码输入框不读取。
取词/粘贴恢复可读取的剪贴板格式；发现剪贴板被其他操作更新时保留新内容。
直接点击“复制”会更新剪贴板。

真实 API 服务与第三方编辑器的兼容性需要分别确认。显式系统 HTTPS 代理
已支持，PAC 代理尚未适配。云端服务的数据处理由该服务商决定。

v0.1 的 API、密钥与快捷键配置继续保留；旧目标语言迁移为外语。
v0.2 的写入模式默认支持整个输入框。

## 许可证与致谢

本项目使用 [MIT License](LICENSE)。感谢
[everettjf/TypeTide](https://github.com/everettjf/typetide) 提供交互参考。
