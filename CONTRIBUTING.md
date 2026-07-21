# CodeSkin 贡献指南

> English version: [CONTRIBUTING.md](docs/en/CONTRIBUTING.md)

感谢你愿意帮助改进 CodeSkin。我们欢迎代码、兼容性测试、文档、翻译、主题和可复现
Bug 报告等贡献。

## 开始前请阅读

请先阅读 [素材与授权说明](NOTICE.md)、[素材政策](ASSET_POLICY.md) 与
[安全政策](SECURITY.md)。提交 PR、Issue 附件、主题或文档时，代表你确认自己有权
分享相关内容，并同意其按照仓库适用的许可证和政策分发。

## 你可以如何贡献

不需要会 Rust 也能帮助项目。你可以：

- 报告安装、连接、注入、还原或兼容性问题；
- 在新的 Windows 或 ChatGPT Desktop 版本上测试；
- 改进中英文文档与故障排除步骤；
- 提交原创或明确可再分发的视觉主题；
- 处理带有 `good first issue` 标签的任务。

## 开发环境

### 前置要求

- Windows
- Node.js 和 npm
- Rust stable 工具链
- 含 MSVC 支持的 Visual Studio C++ Build Tools
- Microsoft Edge WebView2 Runtime

### 安装、构建与测试

```powershell
npm ci
npm run build

cd src-tauri
cargo test
```

构建桌面应用：

```powershell
npm.cmd run build:desktop
```

## 贡献流程

1. 新开 Issue 前先搜索已有 Issue。
2. 对较大的改动，先开 Issue 或 Discussion 讨论。
3. Fork 仓库并创建聚焦的分支。
4. 尽可能让一个 PR 只解决一个逻辑问题。
5. 运行相关构建与测试。
6. 如果行为会影响用户，请同步更新文档。
7. 在 PR 中说明问题、方案、测试步骤和限制。

## PR 检查清单

- [ ] 改动聚焦于真实问题或已记录的需求。
- [ ] `npm run build` 成功。
- [ ] 相关 Rust 测试通过。
- [ ] 已在必要时手动测试新行为。
- [ ] 未提交 API Key、Token、私有路径、聊天内容或敏感日志。
- [ ] 未提交未授权图片、明星图片、品牌素材或来源不清的视觉素材。
- [ ] 必要时已更新文档和截图。

## 主题与图片规则

只有符合[素材政策](ASSET_POLICY.md)的视觉素材才可以加入项目。每个主题素材都要提供
来源、许可证、作者署名和再分发许可；不确定能否再分发时，请不要提交。

## 兼容性报告

兼容性报告尤其有价值。请提供 CodeSkin、Windows、ChatGPT Desktop 版本，
目标检测、连接、应用、还原结果，以及脱敏后的截图或诊断信息。不要提交 API Key、
访问令牌、私有仓库路径、私有任务内容或含敏感信息的截图。

## 社区行为

请保持尊重、建设性与耐心。我们欢迎所有经验水平的贡献者。
