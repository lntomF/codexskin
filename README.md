# CodeSkin

**给 Codex 桌面端换一张会呼吸的脸。**
非官方纯视觉工具 · 本机 CDP 运行时注入 · 不改官方安装包

非 OpenAI 官方产品。不修改 Codex 官方文件、`app.asar` 或代码签名。

## 它能做什么

- **真·可交互**：侧栏、卡片、输入框等都是 Codex 原生控件，不是套壳截图
- **运行时注入**：通过本机 CDP 为 Codex Desktop 动态添加主题与壁纸层
- **可还原**：一键移除注入的样式/壁纸层，随时恢复官方外观
- **相对安全**：CDP 只绑定本机回环地址 `127.0.0.1`，不对外暴露

## 快速开始

目前仅支持 **Windows**（其他平台暂未适配）。

在项目根目录执行：

```powershell
npm.cmd run build:desktop
```

生产程序输出为：

```text
src-tauri\target\release\codeskin.exe
```

可直接运行该 `codeskin.exe`。生产包使用内嵌前端资源，不依赖开发服务器或 `localhost:1420`。

## 使用规则

- 如果 Codex Desktop 已在运行且没有 CDP 端口，请先在其界面中**正常退出**，再由 CodeSkin 启动；不要通过结束进程来处理
- 应用主题后可执行验证；使用“还原”会移除 CodeSkin 注入的壁纸/样式层
- 关闭 CodeSkin 前也应先点击还原

## 壁纸要求

- 支持 PNG、JPEG、WebP
- 单个文件最大 **12 MiB**；图片宽和高均不得超过 **8192 像素**
- 壁纸仅用于 CodeSkin 的临时运行时视觉层；不会改写 Codex 官方资源

## 反馈与贡献

欢迎通过 [Issues](https://github.com/lntomF/codexskin/issues) 反馈 Bug 或提功能建议，也欢迎提交 PR。提交前建议先自行验证一遍“应用主题 → 还原”的完整流程。

## 安全边界

- CDP 只绑定 `127.0.0.1` 回环地址，主题运行期间请勿运行来路不明的本机程序
- 不修改官方安装目录、`app.asar` 或代码签名
- 不读取、不改写 API Key、Base URL 等模型供应商配置

## 免责声明

CodeSkin 是个人维护的非官方项目，与 OpenAI / Codex 官方无关，不提供任何担保；使用本工具产生的风险由使用者自行承担。

---

Star 一下，挑一张喜欢的图，把 Codex 变成今天想要的样子。
