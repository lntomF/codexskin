# Contributing to CodeSkin

> 中文版：[查看中文贡献指南](../../CONTRIBUTING.md)

Thanks for your interest in improving CodeSkin. Contributions are welcome for
code, compatibility testing, documentation, translations, themes, and
reproducible bug reports.

## Before you begin

Please read [NOTICE.md](NOTICE.md), [ASSET_POLICY.md](ASSET_POLICY.md), and
[SECURITY.md](SECURITY.md). By submitting a pull request, issue attachment,
theme, or documentation change, you confirm that you have the right to share
it and that it may be distributed under the repository's applicable license and
policies.

## Ways to contribute

You do not need to write Rust code to help. Useful contributions include:

- reporting installation, connection, injection, restoration, or compatibility
  problems;
- testing CodeSkin with a new Windows or Codex / ChatGPT Desktop version;
- improving Chinese or English documentation;
- adding troubleshooting steps;
- submitting original or clearly licensed visual themes; and
- fixing a labeled `good first issue`.

## Development setup

### Prerequisites

- Windows
- Node.js and npm
- Rust stable toolchain
- Visual Studio C++ Build Tools with MSVC support
- Microsoft Edge WebView2 Runtime

### Install, build, and test

```powershell
npm ci
npm run build

cd src-tauri
cargo test
```

To build the desktop application:

```powershell
npm.cmd run build:desktop
```

## Contribution workflow

1. Search existing Issues before opening a new one.
2. For substantial changes, open an Issue or Discussion first.
3. Fork the repository and create a focused branch.
4. Make one logical change per pull request whenever practical.
5. Run the relevant build and tests.
6. Update documentation when user-visible behavior changes.
7. Explain the problem, solution, test steps, and limitations in the pull
   request.

## Pull request checklist

- [ ] The change is focused and related to a real issue or documented need.
- [ ] `npm run build` succeeds.
- [ ] Relevant Rust tests pass.
- [ ] New behavior has been tested manually where appropriate.
- [ ] No API keys, tokens, private paths, chat content, or sensitive logs are
      included.
- [ ] No unlicensed images, celebrity images, brand assets, or unclear visual
      assets are included.
- [ ] Documentation and screenshots are updated when necessary.

## Theme and image rules

Do not add visual assets unless they follow [ASSET_POLICY.md](ASSET_POLICY.md).
For every submitted theme asset, include its source, license, author
attribution, and redistribution permission. If you are not certain an asset can
be redistributed, do not submit it.

## Compatibility reports

Compatibility reports are especially valuable. Include the CodeSkin version,
Windows version, Codex / ChatGPT Desktop version, target-detection result,
connection result, apply result, restore result, and sanitized screenshots or
diagnostics when available. Never include API keys, access tokens, private
repository paths, private task content, or screenshots containing sensitive
information.

## Conduct

Be respectful, constructive, and patient. We welcome contributors of all
experience levels.
