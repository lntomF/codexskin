# CodeSkin

CodeSkin 是面向 Codex Desktop 的**非官方纯视觉工具**：通过本机 CDP 在运行时注入主题和壁纸层，不修改 Codex 官方文件、`app.asar` 或签名。

## Windows 构建与运行

在项目根目录执行：

```powershell
npm.cmd run build:desktop
```

生产程序输出为：

```text
src-tauri\target\release\codeskin.exe
```

可直接运行该 `codeskin.exe`。生产包使用内嵌前端资源，不依赖开发服务器或 `localhost:1420`。

## 本机 CDP 与连接规则

- CDP 只连接到 `127.0.0.1` 回环地址，绝不对外暴露。
- 如果 Codex Desktop 已在运行且没有 CDP 端口，请先在其界面中**正常退出**，再由 CodeSkin 启动；不要通过结束进程来处理。
- 应用主题后可执行验证；使用“还原”会移除 CodeSkin 注入的壁纸/样式层和刷新注入。关闭 CodeSkin 前也应先还原。

## 壁纸

- 支持 PNG、JPEG、WebP。
- 单个文件最大 **12 MiB**；图片宽和高均不得超过 **8192 像素**。
- 壁纸仅用于 CodeSkin 的临时运行时视觉层；不会改写 Codex 官方资源。
