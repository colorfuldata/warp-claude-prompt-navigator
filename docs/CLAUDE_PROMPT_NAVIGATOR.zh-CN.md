# Warp OSS 的 Claude Code 提问导航

[English](CLAUDE_PROMPT_NAVIGATOR.en.md) · [返回首页](../README.md)

## 源码基线

本项目基于 [Warp 官方开源仓库](https://github.com/warpdotdev/warp) 的提交 [`df5cacf`](https://github.com/warpdotdev/warp/commit/df5cacf)（2026-09-24）。请使用提交号识别基线；`app/Cargo.toml` 中的 `0.1.0` 是 Cargo 包版本，并非对应的 Warp 正式发布版本。本改版不是 Warp 官方发行版，也没有修改官方 Warp 安装程序。

## 新增功能

- 在 Warp 图形界面中运行 Claude Code 时，终端右侧显示当前会话每次人工提问的刻度。刻度紧凑排列，并在可用区域垂直居中；最新刻度使用高亮色。
- 鼠标移过刻度时，相邻刻度呈波浪式伸缩，同时显示该次提问的内容预览。预览不显示“第 N 次提问”之类的编号。
- 点击刻度尝试跳转到终端滚动历史中的对应提问。如果原始终端内容已被截断，可点击查看从 Claude Code 本地会话记录恢复的完整提问内容。
- 仅对 Warp 识别到的 Claude Code CLI 会话生效；没有提问时不显示导航条。窄窗口会隐藏导航条。

## 安装与使用

1. 安装此仓库构建的 Warp OSS 版本和 Claude Code CLI。Windows 测试环境将此改版安装在 `D:\WarpPromptNavigator\WarpOss`；请不要直接覆盖已有的官方 Warp 安装。
2. 在 Claude Code 中安装 Warp 官方的 [Claude Code + Warp 插件](https://github.com/warpdotdev/claude-code-warp)：

   ```text
   /plugin marketplace add warpdotdev/claude-code-warp
   /plugin install warp@claude-code-warp
   ```

   插件还需要 `jq`。安装后重启 Claude Code，或运行 `/reload-plugins`。
3. 在此改版 Warp 的终端中启动 `claude`，连续发送几次提问。右侧会出现刻度；移入查看内容，点击定位。

导航依赖插件发出的 `UserPromptSubmit` 等会话事件。只安装 Claude Code CLI、不安装插件时，Warp 无法可靠获得每次提问。会话记录来自本机 Claude Code 的 `~/.claude/projects`，或 `CLAUDE_CONFIG_DIR/projects`；本功能不会上传该记录。

## 从源码构建

克隆本仓库后，在仓库根目录按 Warp 官方 [开发指南](../CONTRIBUTING.md) 安装平台依赖，并构建 OSS 图形界面：

```bash
cargo build --release --bin warp-oss
```

Windows 打包工具与依赖参见 [`script/windows/README.md`](../script/windows/README.md) 和 [`script/windows/bundle.ps1`](../script/windows/bundle.ps1)。`script/windows/vm-import-prompt-navigator.ps1` 是本项目的虚拟机源码导入辅助脚本，需要配套的本地源码压缩包，并非安装程序。仓库只包含源码；此前测试用的安装包不包含在 Git 提交中。

## 已知限制与验证

- 只显示当前 Warp 图形终端中活动的 Claude Code 会话；重启 Warp 后不会自动恢复此前导航条。
- 跳转依赖 Warp 尚保留对应终端历史。历史被截断时，点击刻度会显示提问预览；无法保证滚动到已不存在的终端行。
- Windows 11 虚拟机中已手动验证 16 次提问、右侧刻度、悬停预览与波浪动效。macOS 安装包尚未构建或验证。

## 许可

沿用上游许可：`warpui_core` 和 `warpui` 采用 [MIT](../LICENSE-MIT)；其余 Warp 源码采用 [AGPL-3.0](../LICENSE-AGPL)。Claude Code + Warp 插件属于[独立仓库](https://github.com/warpdotdev/claude-code-warp)，本仓库没有复制其源码。
