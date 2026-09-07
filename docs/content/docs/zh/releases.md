---
title: 版本与发布
description: 产品版本与协议版本、预览版下载、校验升级，以及维护者的版本准备和发布流程。
order: 10
---

# 版本与发布

WVST 使用统一产品版本：Rust workspace、Bridge、host worker、Web SDK、示例和文档包一起升级。当前首个打包版本为 `0.1.0-alpha.2`。文档导航显示当前构建的版本；历史源码和对应文档保留在 Git tag 中。

## 产品版本与协议版本

| 标识 | 用途 |
| --- | --- |
| `0.1.0-alpha.2` | 产品版本，采用 SemVer。alpha/beta/rc 表示预览阶段。 |
| `v0.1.0-alpha.2` | Git tag，与具体源码提交、GitHub Release 对应。 |
| 控制协议、音频帧、worker IPC 版本 | 数据兼容性，按协议变化独立演进；不随每次产品升级增加。 |
| 状态快照与诊断 schema | 对应数据结构的兼容性，不等于 SDK 版本。 |
| Svedocs 版本 | 文档框架依赖，独立于 WVST 产品版本。 |

修复通常增加 patch，新功能增加 minor；1.0 以前，不兼容 API 变化也增加 minor 并写明迁移步骤。预览版按 `alpha.1 → alpha.2 → beta.1 → rc.1` 推进。不要复用已发布 tag 或覆盖已发布附件。

## 下载适合你的产物

从 [GitHub Releases](https://github.com/backrunner/wvst/releases) 选择明确标记的预览版本。草稿仅维护者可见，尚未构成对外发布；发布工作流通过后才具备可审核的完整产物。

| 文件名中的目标 | 使用场景 |
| --- | --- |
| `aarch64-apple-darwin` | Apple Silicon macOS。 |
| `x86_64-apple-darwin` | Intel macOS。 |
| `x86_64-unknown-linux-gnu` | x64 Linux 实验运行包，构建基线 Ubuntu 22.04。 |
| `x86_64-pc-windows-msvc` | x64 Windows 实验运行包。 |
| `wvst-web-VERSION.tgz` | 对应版本的 SDK，可用 `npm install ./wvst-web-VERSION.tgz` 安装。 |

原生文件名包含版本和目标，例如 `wvst-0.1.0-alpha.2-aarch64-apple-darwin.tar.gz`。macOS 是优先插件运行目标；能下载 Windows/Linux 构建不代表已经验证所有第三方插件。已发布的 alpha.2 压缩包未签名。配置好仓库凭据后，后续 macOS 版本将使用 Developer ID 签名、Apple 公证的 `.dmg`；Windows/Linux 仍未签名。请以具体 Release 的附件和签名状态为准。

## 校验与启动

下载目标压缩包、`SHA256SUMS` 和 `release-manifest.json`。校验工具会对未下载的其他平台附件提示缺失；只比较所选文件对应行即可。macOS 可以使用：

```sh
shasum -a 256 wvst-0.1.0-alpha.2-aarch64-apple-darwin.tar.gz
```

Linux 可使用 `sha256sum`，Windows 使用 `Get-FileHash -Algorithm SHA256`。摘要应与 `SHA256SUMS` 完全一致。`release-manifest.json` 记录来源 commit、版本、文件大小和 hash。校验和用于发现损坏，不是代码签名。

对于已签名 macOS 版本，打开 DMG，将其中整个 WVST 文件夹复制到本地可写目录，再推出磁盘映像；tar/zip 则解压整个目录。按照包内 README 启动。包中 `bin` 包含同一版本的 Bridge 和 worker，`wvst-runtime.json` 记录运行包身份。程序可在不启动服务的情况下报告版本：

```sh
./bin/wvst-bridge-server --version
./bin/wvst-host-worker --version
```

Windows 对应文件名带 `.exe`。运行包不安装系统服务；启动时需要配置 token，并在 Studio 高级设置输入同一个值。与源代码构建相同，插件需自行安装。

## 升级与回退

停止旧 Bridge 和插件实例，解压新包到独立目录，使用新包内的 Bridge/worker 组合重新启动。保留旧目录以便回退；不要只替换其中一个可执行文件。SDK 优先使用匹配产品版本，并通过握手检查协议兼容性。

插件状态由应用保存。升级前保留状态快照与原插件版本，阅读 CHANGELOG 的 breaking changes；产品版本相同不保证第三方插件的私有状态跨版本兼容。

## 维护者：准备一个版本

`Cargo.toml` 的 `workspace.package.version` 是产品版本来源。不要手工分别改各 package.json。

1. 在根目录 `CHANGELOG.md` 的 `[Unreleased]` 下记录用户可见变化、修复、兼容性和迁移事项。
2. 在仓库根目录运行：

```sh
npm run release:check
npm run release:prepare -- 0.1.0-alpha.3 --date 2026-09-08
npm run release:check -- --tag v0.1.0-alpha.3
npm run release:notes -- 0.1.0-alpha.3
```

这里的版本与日期仅为下一次发布示例，实际发布时按计划修改。prepare 会更新 Rust/npm manifests、Cargo.lock/package-lock.json、生成的 SDK 版本，并把 Unreleased 归档为带日期的发布说明。所有修改先解析验证，写入失败时尝试回滚。

`npm run release:sync` 用于修复 workspace 版本漂移，不会创建新发布记录。正常升级使用 prepare。工具不自动提交或推送 tag；先 review diff 并通过项目检查。

## 维护者：从 tag 到发布

版本提交推送、CI 通过后，创建不可复用的注释 tag：

```sh
git tag -a v0.1.0-alpha.3 -m 'WVST 0.1.0-alpha.3'
git push origin v0.1.0-alpha.3
```

`Release Preview` 工作流校验 tag/版本/发布说明，复用完整 CI，再构建四个原生运行包和 SDK tarball。只有全部成功才生成 SHA256SUMS、来源 manifest 和 GitHub Release 草稿。重跑可更新同一个草稿，已公开 Release 不允许覆盖。

维护者下载附件校验、确认平台说明和迁移内容后，将草稿发布为 prerelease。当前流水线只接受预览版 tag；正式版本还需要接入平台签名、公证和对应源码提交的真实插件/长期稳定性证据，不能把“CI 编译通过”当成正式兼容性认证。

源码与 npm registry 发布是独立动作；本流程不会执行 `npm publish` 或 `cargo publish`。文档站更新通过 `npm run docs:deploy` 单独执行。[开发与验证](/docs/zh/development)说明不同证据的覆盖范围。

## 维护者：配置 macOS 签名

推送新版本标签之前，在 **backrunner/wvst** 仓库配置以下 Actions Secrets。AIPass 仅作为流程参考：GitHub 不允许读回它的 Secret 值，也不会自动在两个仓库间共享。

| Secret | 内容 |
| --- | --- |
| `APPLE_CERTIFICATE` | 指定 Developer ID Application 证书及配对私钥的 P12，经过 base64 编码。 |
| `APPLE_CERTIFICATE_PASSWORD` | 这份 P12 的导出密码。 |
| `APPLE_SIGNING_IDENTITY` | 与证书一致的完整 Developer ID Application 身份名称。 |
| `APPLE_TEAM_ID` | 与证书一致的开发者团队 ID。 |
| `APPLE_ID` | 有权使用该团队的 Apple 账号。 |
| `APPLE_PASSWORD` | Apple 应用专用密码，与登录密码和 P12 密码不同。 |

本机 Developer ID 身份已对 WVST 二进制完成签名验证。这不代表 GitHub Secrets 已配置或公证已通过。alpha.2 的公开附件保持不变；补齐凭据并通过签名流程后，发布新版本。

工作流将证书导入 runner 钥匙串。Rust `notary-credentials` 命令通过 stdin 将应用专用密码交给 `notarytool`，验证账号后保存临时 profile；`bundle-macos` 通过该 profile 认证，不将密码放入命令参数。WVST 不需要 AIPass 的 CloudKit provisioning profile 或 Tauri 更新密钥。

Bridge 和 worker 都使用 hardened runtime 和安全时间戳。只有 worker 获得 `com.apple.security.cs.disable-library-validation`，以加载其他开发者的 VST 动态库；Bridge 保持库校验。两者都不授予调试或 JIT 权限，需要额外例外的插件须单独验证。

打包工具在**签名后**计算二进制哈希，并校验版本和 CPU 架构。随后签名 DMG、要求 Apple 明确返回 `Accepted`、附加并验证公证票据、执行 Gatekeeper 检查，再只读挂载 DMG，将实际包内的程序、元数据和许可证与打包目录比对。工作流保留 `signing-TARGET` 报告，记录公证提交 ID、来源 commit 和最终 DMG 哈希。缺少凭据、公证拒绝或任何校验失败都会阻止创建 Release。

本地手动打包可使用已有的 `notarytool` 钥匙串 profile：

```sh
# 在本机配置 APPLE_SIGNING_IDENTITY、APPLE_TEAM_ID、WVST_NOTARY_PROFILE。
# 先构建对应架构的两个运行程序。
cargo run --locked -p wvst-packager --bin wvst-release -- bundle-macos \
  aarch64-apple-darwin target/aarch64-apple-darwin/release output/release FULL_COMMIT_SHA
```

命令需要新的 `.wvst-macos-TARGET` 暂存目录，并拒绝覆盖已有 DMG。不要将凭据写入源码、命令历史或聊天。下载 DMG 后，可在 macOS 验证：

```sh
codesign --verify --strict --verbose=2 /path/to/wvst-VERSION-TARGET.dmg
xcrun stapler validate /path/to/wvst-VERSION-TARGET.dmg
spctl --assess --type open --context context:primary-signature --verbose=4 /path/to/wvst-VERSION-TARGET.dmg
```

裸命令行程序不能附加公证票据，因此分发和保留 DMG。签名、公证验证分发身份，不替代真实插件兼容性及音频稳定性测试。来源清单的 `signingPolicy` 表示发布要求；仅校验 SHA256SUMS 不能验证 Apple 签名。
