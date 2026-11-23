# OpenAPI demo 验证基线归档

归档日期：2026-11-17。用户在当前会话确认 demo 已验证，结合前文 Windows demo 上下文，记录为用户确认 Windows demo 验证通过。这里不将该确认表述为代理执行了 Windows 实机测试，也不将只读查询通过扩展为上传、发布权限通过。

## 内容

- 本目录的 `source/`：归档开始时复制的 demo 模板、说明、验证与打包脚本及 Rust SDK 源码、Cargo 清单和锁文件。
- `manifest.json`：来源说明、大小、SHA-256、仓库相对位置和验证范围。
- 仓库 `artifacts/archive/openapi-demo-2026-11-17/`：原三平台 demo ZIP、独立客户端、逐包校验文件和 `SHA256SUMS`。大文件保存在已忽略的产物目录，不放入源码归档目录。
- 仓库 `dist/mpaas-openapi-demo-archive-2026-11-17.zip`：供下载留存的归档包，包含本说明、清单、只读模板和三个原 demo ZIP。

## 使用与边界

下载归档 ZIP 后，在 `packages/` 中选择对应平台的原 demo ZIP 再解压，按其 README 填写本地配置和配套私钥路径。Windows 包仅查询 `getPackages`，不编译、不上传、不发布。

归档保留原交付文件字节，因此其中 README 关于代理当时未完成 Windows 实机验证的描述保持原样；用户后续验证确认记录在本说明与清单中。

归档未包含任何本地配置、密钥或真实后台参数。源文件快照用于审计参考，未执行可重现构建；二进制基线以清单内各自 SHA-256 为准。现有 `demo/openapi` 和验证脚本继续保留，可用于后续回归验证。
