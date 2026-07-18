# CodeSkin

**为你的 Codex 桌面端，披上一件随呼吸律动的数字化外衣。**

- **动态视觉美学**：打破死板的暗黑/明亮模式，为 Codex 注入会呼吸的动效与精巧的图层微动，让生产力工具兼具艺术感。

- **沉浸式主题定制**：深度重构视觉层。无论是磨砂玻璃的通透，还是赛博朋克的霓虹，皆在不改变官方原有交互逻辑的前提下完美呈现。

- **无感优雅常驻**：基于本机 CDP 运行时渲染技术，将定制主题直接映射至界面。无需解包或修改官方原生文件，开箱即美，安全无痕。

## 赞助商

<p align="center">
  <a href="https://api.shunyin.eu.cc/sign-up?aff=P5tA">
    <img src="src-tauri\icons\sponsor-xiai.png" width="200" alt="XIAI中转">
  </a>
</p>

<p align="center">
  <strong>智能互联 · 自由创造</strong><br>
  <sub>Smarter Links, Freer Creation</sub>
</p>

<p align="center">
  感谢 <a href="https://api.shunyin.eu.cc/sign-up?aff=P5tA"><strong>XIAI中转</strong></a> 赞助本项目。<br>
  满血 AI 中转：官方模型直连，无降智、无套壳；一行配置接入 Codex / Claude Code。
</p>

<p align="center">
  <sub>
    换肤与 API 配置互相独立，本项目不会自动改写你的模型供应商设置。
  </sub>
</p>

## 你可以用它做什么

- **无缝原生体验**：从输入框到侧边栏，保留 Codex 原生丝滑交互，这不是一张死板的壁纸，而是你的定制版桌面
- **无感动态加载**：利用本机 CDP 技术，在软件运行时魔改主题，不污染任何本地文件。
- **来去自如的切换**：喜欢就用，厌了就换。一键卸载皮肤层，秒回官方初始状态。
- **严苛的安全防线**：注入通道仅对内（127.0.0.1）开放，不给外部网络留任何漏洞，守护你的数据安全。

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

## 主题保存与运行限制

CodeSkin 会保存你最近选择的主题/壁纸信息；但**壁纸显示本身是通过 CDP 注入到当前运行中的 Codex 窗口的运行时效果**，不是写入 Codex 官方文件或安装目录。

因此请注意：

- CodeSkin 正在运行时，它可以连接 Codex 并应用已保存的主题。
- 如果你**完全退出 CodeSkin**，随后又从原生 Codex 图标、命令行或其他入口启动一个新的 Codex，新的窗口不会自动带上背景。这不代表主题配置丢失，而是没有运行中的 CodeSkin 为新进程执行 CDP 注入。
- 若要再次显示已保存的主题，请先重新启动 CodeSkin，并通过它重新连接/应用主题到当前 Codex。
- CodeSkin 不会修改 Codex 官方文件、`app.asar`、安装目录或代码签名；因此不提供“CodeSkin 已退出后仍永久改写新 Codex 窗口外观”的能力。

> 想让每次新开的 Codex 自动恢复主题，需要让 CodeSkin 保持运行以监听并注入新窗口，或使用未来可能提供的“带主题启动 Codex”专用启动入口。当前普通的原生 Codex 启动方式不会自动触发注入。

## 使用规则

- 如果 Codex Desktop 已在运行且没有 CDP 端口，请先在其界面中**正常退出**，再由 CodeSkin 启动；不要通过结束进程来处理
- 应用主题后可执行验证；使用“还原”会移除 CodeSkin 注入的壁纸/样式层
- 如需恢复官方外观，请在 CodeSkin 中点击“还原”；单纯退出 CodeSkin 不会把已注入到当前 Codex 窗口的运行时样式写入官方文件

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
