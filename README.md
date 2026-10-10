# RustTide

使用 **Rust + Tauri** 重写的 macOS 翻译工具，灵感来自
[TypeTide](https://github.com/everettjf/TypeTide)。采用 **MIT License**。

> v0.1.0：macOS API 版已可本地构建和运行。尚未提供经过 Apple Developer ID
> 签名与公证的发行包。

![RustTide macOS 翻译工作台](docs/screenshots/macos.png)

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

## 使用方法

1. 启动 RustTide，进入“设置”，填写 API 地址、模型和密钥并保存。
   远程服务使用 HTTPS，例如 `https://api.openai.com/v1`。也可使用本机
   OpenAI 兼容服务的 HTTP 地址。密钥按地址分别保存到 macOS 钥匙串。
2. 点击“测试已保存的 API”，验证配置。测试只发送示例文字“你好，世界！”。
3. 在系统设置 → 隐私与安全性 → 辅助功能中允许 RustTide。
4. 在其他应用中选中文字，按 **Option+D** 取词并预览翻译；确认后点击
   “替换原文”，或复制译文。**Option+R** 会翻译并直接替换当前选区。
5. 也可手动输入原文，点击“翻译”。“3 秒后取词”给你时间切换到目标应用。

译文可编辑。修改原文会解除原输入框的关联，避免误替换。默认只读取选中
文字；“没有选区时读取整个输入框”需要在设置中开启。

替换前核对原应用、输入框、原文和选区。发生变化时拒绝替换，需要重新
取词。剪贴板取词和粘贴会保存并恢复可读取的全部剪贴板格式；如果发现
剪贴板被其他操作更新，则保留新内容。直接复制译文会更新剪贴板。

关闭窗口后应用继续在菜单栏运行，使用菜单栏“退出 RustTide”结束运行。
默认语言和风格在翻译页选择后，通过“设置 → 保存设置”持久化。

## 本地开发与打包

需要 macOS 12 或更高版本、Xcode Command Line Tools、Rust 1.90 或更新版本、
Node.js 和 npm。开发机已验证 Apple Silicon macOS；其他架构尚未实测。

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

构建结果：`src-tauri/target/release/bundle/macos/RustTide.app`。
Tauri 使用系统 WebView，无需独立浏览器运行时或本地 HTTP 服务。

## 已验证与限制

- 前端状态回归、Rust API 请求/错误/取消回归和静态检查。
- 原生取词、粘贴、原文变化拦截、密码框保护、剪贴板恢复及撤销。
- 实际 Tauri 应用通过快捷键调用本地模拟 API，并完成预览和替换。
- 密钥的隔离写入、读取和删除通过；测试未使用真实 API 密钥。

不同编辑器的 Accessibility 支持不同。无法识别或验证输入框时，请手动
粘贴原文或复制译文；只读内容不允许原位替换。真实服务商和所有第三方
编辑器的兼容性仍需逐一验证。系统代理支持显式 HTTPS 代理，未适配 PAC。

原文只在主动翻译时发给配置的 API。本应用不保存翻译历史，不记录密钥或
原文日志。服务商对请求数据的处理由其自己的政策决定。

开发与验证细节见 [验证记录](docs/verification.md)。

详细范围见 [实施计划](docs/implementation-plan.md)。

## 许可证与致谢

本项目使用 [MIT License](LICENSE)。感谢
[everettjf/TypeTide](https://github.com/everettjf/TypeTide) 提供参考。
