# Claude Code Prompt Navigator for Warp OSS

[简体中文](CLAUDE_PROMPT_NAVIGATOR.zh-CN.md) · [Home](../README.md)

## Problem it solves

In a long Claude Code conversation inside Warp, replies and tool output keep extending the terminal history. Finding a prompt you entered several turns ago means repeatedly scrolling to locate its original text. This project gathers the current session's human prompts in a right-side rail, so you can preview an earlier instruction on hover and try to jump to it with a click.

## Source baseline

This project is based on commit [`df5cacf`](https://github.com/warpdotdev/warp/commit/df5cacf) (2026-09-24) of the [official open-source Warp repository](https://github.com/warpdotdev/warp). Use the commit hash to identify the exact baseline. The `0.1.0` in `app/Cargo.toml` is a Cargo package version, not a corresponding official Warp release version. This modification is not an official Warp release and does not alter the official Warp installation.

## Added features

- While Claude Code runs in the Warp GUI, a right-side rail shows one tick for each human prompt in the current session. Ticks are compact and vertically centered in the available area; the newest tick is highlighted.
- Hovering over a tick expands nearby ticks in a wave and previews the prompt text. The preview has no “Prompt #N” heading.
- Clicking a tick attempts to scroll to that prompt in terminal history. If the terminal output has been truncated, clicking can show the full prompt recovered from Claude Code's local transcript instead.
- The rail appears only for CLI sessions identified by Warp as Claude Code sessions and stays hidden until there is a prompt. It is hidden in narrow panes.

## Installation and use

1. Install a Warp OSS build from this repository and the Claude Code CLI. Keep this modified build separate from any existing official Warp installation.
2. In Claude Code, install the official [Claude Code + Warp plugin](https://github.com/warpdotdev/claude-code-warp):

   ```text
   /plugin marketplace add warpdotdev/claude-code-warp
   /plugin install warp@claude-code-warp
   ```

   The plugin also requires `jq`. Restart Claude Code or run `/reload-plugins` after installation.
3. Start `claude` in this modified Warp terminal and submit several prompts. Hover over the right-side ticks to preview prompts; click a tick to navigate.

The navigator depends on session events such as `UserPromptSubmit` emitted by the plugin. The CLI alone does not give Warp a reliable record of each prompt. Local transcripts are read from Claude Code on your computer. This feature does not upload them.

## Build from source

Clone this repository, install the platform dependencies in Warp's [contribution guide](../CONTRIBUTING.md), and build the OSS GUI from the repository root:

```bash
cargo build --release --bin warp-oss
```

For Windows packaging dependencies and tools, see [`script/windows/README.md`](../script/windows/README.md) and [`script/windows/bundle.ps1`](../script/windows/bundle.ps1). `script/windows/vm-import-prompt-navigator.ps1` is a VM source-import helper that requires a matching local source archive; it is not an installer. Git contains source only; the installer used for testing is not committed.

## Limitations and verification

- The rail covers only the active Claude Code session in the Warp GUI. It does not automatically restore the previous rail after restarting Warp.
- Scrolling to a prompt requires its terminal history to remain available. If history was truncated, clicking shows a prompt preview; it cannot scroll to a line that no longer exists.
- The Windows 11 VM was manually checked with 16 prompts, the right-side rail, hover preview, and wave animation. A macOS installer has not yet been built or verified.

## License

The upstream licenses remain in force: `warpui_core` and `warpui` are [MIT](../LICENSE-MIT); the rest of the Warp source is [AGPL-3.0](../LICENSE-AGPL). The Claude Code + Warp plugin is a [separate repository](https://github.com/warpdotdev/claude-code-warp); its source is not copied here.
