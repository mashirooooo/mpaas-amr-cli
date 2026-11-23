# mPaaS 小程序编译与发布工具

将 minidev 编译器、Node 运行时和 Rust 私有云 OpenAPI 客户端封装为单文件，提供编译、打包、上传和发布功能。

交付文件可以直接运行；以下 npm/Cargo 命令用于源码开发和生成交付文件。

最终用户操作见 [使用说明](docs/usage.md)。当前只验证 OpenAPI 时，使用 [只读查询 demo](demo/openapi/README.md)；它不编译、不上传、不发布，也可以用独立 Rust 客户端运行。

用户已确认通过的 demo 已[归档](archive/openapi-demo-2026-11-17/README.md)。正式功能与自动版本规则见 [自动上传与发布](docs/publishing.md)：上传前查询后台最新版本，首次为 `0.0.1`，按 `0.0.19 → 0.1.0` 进位，首位不增加。

上传业务字段默认从上一版资源和小程序应用信息填充，本次显式配置优先。完整 `mpaas-amr` 工具的 `upload` 可以接收 minidev 源码目录；检测到 `mini.project.json`、`.axml`、`.acss` 或 `.less` 时，会先用内置编译器生成 TAR，再生成符合接口要求的 ZIP 后上传。已经编译但尚未压缩的目录和已有 ZIP 也继续兼容。名称、虚拟域名、客户端版本范围、入口、开关与扩展参数按需继承。可用 `getUploadDefaults` 只读预览。

## 安装

```bash
npm ci
```

## 构建 ZIP 与 AMR

```bash
npm run build
npm run validate
```

也可以指定实际项目：

```bash
node src/cli.mjs build \
  --project /path/to/mini-project \
  --app-id 0000000000000012 \
  --version 1.0.24.0 \
  --output artifacts \
  --minify \
  --parallel
```

输出文件：

```text
artifacts/1.0.24.0/0000000000000012.amr
artifacts/1.0.24.0/0000000000000012.zip
artifacts/1.0.24.0/0000000000000012.tar
artifacts/1.0.24.0/0000000000000012.build.json
```

## 离线资源

```bash
npm run download-assets
```

带构建器：

```bash
node src/cli.mjs download-assets --with-compiler
```

执行 `npm run package:sea` 时，脚本会把以下内容作为内置资源打入单文件：

```text
node_modules/             minidev 及其运行时依赖
.minidev/compilers/       当前平台的 mini-pkg-builder、esbuild、packer
minidev/assets/           minidev 离线资源
bin/                     本平台 Rust OpenAPI 客户端
```

首次运行单文件时，资源会释放到系统临时目录，后续构建不需要 Node、minidev 安装包或联网下载构建器。打包前必须在目标平台执行一次 `download-assets --with-compiler`。

## 独立可执行文件

在 Linux x64 或 Windows x64 目标环境上执行：

npm run package:sea
./dist/mpaas-amr-<platform>-<arch> --help
./dist/mpaas-amr-darwin-arm64 build \
  --project ./fixtures/minimal \
  --app-id 0000000000000012 \
  --version 1.0.25.0 \
  --output artifacts


```bash
npm run package:sea
./dist/mpaas-amr-<platform>-<arch> --help
./dist/mpaas-amr-<platform>-<arch> build \
  --project /path/to/mini-project \
  --app-id 0000000000000012 \
  --version 1.0.24.0 \
  --output artifacts
```

产物按平台和架构命名：

```text
dist/mpaas-amr-linux-x64
dist/mpaas-amr-win32-x64.exe
```

每个文件都是对应平台的单文件可执行程序。CI 交付目标为 Linux x64 和 Windows x64；macOS arm64 支持本机打包与验证。独立文件需要能够执行 `build` 和 `validate`，并生成符合样例结构的 `.amr`：

```text
<appId>.amr (ZIP)
├── Manifest.xml
├── CERT.json
└── <appId>.tar
```

`package:sea` 使用当前机器的 Node SEA 能力打包。必须分别在 Linux x64 和 Windows x64 构建，或者使用项目内的 CI matrix：

```text
Linux   -> mpaas-amr-linux-x64
Windows -> mpaas-amr-win32-x64.exe
macOS   -> mpaas-amr-darwin-arm64（本机验证）
```

Linux 可在 Docker 中验证：

```bash
docker build --platform linux/amd64 -f docker/linux.Dockerfile -t mpaas-amr-linux .
```

Windows 需要在 Windows Docker 主机或 Windows CI runner 中执行同等流程；Linux Docker 主机不能运行 Windows 容器和 Windows 构建器。

项目内的 `.github/workflows/build.yml` 会分别在 `ubuntu-22.04` 和 `windows-2022` 上下载构建器、生成单文件并执行验证。macOS arm64 可在本机构建。

CI 还会在独立环境中验证单文件：Linux 使用不带 Node.js、项目 `node_modules` 和外部 `.minidev` 的 `debian:bookworm-slim` 容器；Windows 使用全新的临时目录。两个环境都会执行 `--help`、`build` 和 `validate`，因此构建 job 通过并不等于纯净验证通过，发布 job 只有在纯净验证通过后才会执行。

CI 会把完整的 Linux 和 Windows 单文件发布到对应的 GitHub Release；Release 适合保存这类大文件，下载后可以直接运行。`.amr` 只在 CI 临时工作区用于验证，不作为上传附件。`build-artifacts` 分支仅用于保存可审计的单文件分片归档和清单。

由于 GitHub 单个 Git 文件限制为 100 MB，`build-artifacts` 分支不直接提交完整可执行文件，而是保存小于 100 MB 的可执行文件分片和 SHA-256 清单。恢复单文件时，在对应目录执行清单中的 `restore` 命令，然后按清单校验 SHA-256。

## 编译、上传并发布

发布功能以提供的 MDS 私有云 Java 示例和接口文档为准，已经重新实现 Rust SDK。PDF 和 Java Demo 中的接口统一登记在 [serde API 目录](docs/api-reference.md)，请求模型和响应模型集中在 `rust-sdk/src/models.rs`。旧版 Rust 的完整 URL 签名与 `getPackagesUrl` 配置已弃用。

交付机器只需要对应平台的单文件和自己的项目、图标、配置、RSA 私钥，不需要安装 Node、Java、Rust、OpenSSL 或 minidev。编译器和 Rust 客户端首次运行时释放到用户隔离的系统临时目录；上传发布需要访问自己的 mPaaS 后台。构建机器仍需 Node 22.14.0、npm 和 Rust。

发布前应已在后台创建对应小程序，并配置资源虚拟域名与用于 OpenAPI 验签的公钥。

复制 `config/release.example.json` 为 `config/release.local.json`，填写：

- `baseUrl`：mcube OpenAPI 服务地址，如 `https://mcube.example.internal:8281`，只接受协议、主机和端口，省略路径、查询参数。
- `appId`、`workspaceId`、`tenantId`：mPaaS 主应用和工作空间信息；`build.appId` 是另一个概念，表示 16 位小程序 ID。
- `privateKeyFile`：RSA 2048 私钥文件，配对公钥应已配置到 MappCenter。支持 PKCS#8/PKCS#1 PEM，以及 Java 示例使用的裸 Base64 PKCS#8。
- `build`：小程序工程、ID、名称及输出路径；`release` 自动查询下一版本，无需填写固定 `version`。
- `upload`：本地图标，以及按需覆盖的业务字段。虚拟域名、平台和客户端版本范围等默认继承；首次上传需补足仍缺失的必填项。`clientVersionMin` 沿用目标服务的版本约定：旧版本为 `iOS;Android`，概述文档 2.59.0 起新增 Harmony 段。
- `publish`：正式发布为 `publishType: 3`，省略 `publishMode`；灰度发布为 `publishType: 2` 并指定对应模式及规则。

所有相对路径均相对于配置文件目录；字符串支持 `${ENVIRONMENT_VARIABLE}`，环境变量在解析 JSON 后展开，Windows 路径不会被当作 JSON 转义。Java 示例的 `loginName` Cookie 可通过可选 `loginName` 配置，客户端不写死示例用户。

只读查询下一版本，再编译并检查产物，不上传或发布：

```bash
./dist/mpaas-amr-linux-x64 release config/release.local.json --prepare-only
```

编译、上传并创建/启动发布任务：

```bash
./dist/mpaas-amr-linux-x64 release config/release.local.json
```

Windows 使用对应的 `.exe`。`release` 支持正式功能包（`packageType=1`），调用顺序为：

```text
查询下一版本 → minidev Publish → TAR → <小程序ID>.zip
                        ↓
            复查版本 → uploadPackage → packageId
                        ↓
            createNebulaTask → taskId
                        ↓
            getTaskDetail：已发布中则完成；待发布则启动
                        ↓
            changeTaskStatus(1) → getTaskDetail 确认发布中
```

上传直接使用 ZIP。AMR 同时作为兼容产物生成，不作为 `resourceFile` 发送。入口和 `extendInfo.launchParams` 读取编译器的 `mini.built.json.packageInfo`，随 `<小程序ID>.build.json` 保存，避免固定首页或丢失运行时参数。当前编译路径生成整包，不启用尚未确认协议的 Cube/分包上传扩展。

每次请求使用 RSA SHA-256/PKCS#1 v1.5 签名，小写 Hex 编码；签名原文中的 `url` 为 `/openapi/...` 接口路径。公共参数在 URL 上发送，POST 同时使用包含公共参数的 multipart/form-data。完整响应必须满足 `data.success=true`，实际数据从 `data.data` 读取。HTTP 200 的 HTML 登录页、业务失败、重定向都不会被报告为成功。

写请求不自动重试。上传后若创建/启动任务失败，结果保留已取得的 `packageId`、`taskId` 和失败阶段。先查询已有资源/任务再恢复，避免重复上传同版本或重复创建任务。最终 `taskStatus=1` 表示后台任务已进入“发布中”，客户端安装与运行结果需要在目标环境验收。

已有 ZIP 可以执行 `upload <配置>` 只上传，或执行 `upload <配置> --publish` 自动上传发布。已有资源执行 `publish <配置>`；提供 `taskId` 时恢复已有任务，已发布中的任务只查询确认。Windows 独立生产包 `mpaas-publisher-win32-x64.zip` 内置同一 Rust 客户端，直接运行 `mpaas-openapi-rs.exe <配置>`，无需完整编译器。

## 单独使用 Rust OpenAPI 封装

源码开发：

```bash
npm run build:openapi
node src/cli.mjs openapi rust-sdk/config.example.json
```

交付文件内置相同的 Rust 客户端：

```bash
./dist/mpaas-amr-linux-x64 openapi --client-help
./dist/mpaas-amr-linux-x64 openapi rust-sdk/config.get-packages.example.json
```

独立 Rust 程序也可以运行，只接收一个 JSON 配置路径：

```bash
rust-sdk/target/release/mpaas-openapi-rs config.json
```

可用 `action`：

| action | 用途 |
| --- | --- |
| `uploadAndPublish` | 上传已有 ZIP 并创建/启动任务 |
| `upload` | 仅上传已有 ZIP |
| `publish` | 发布已有资源；可按 `taskId` 恢复任务 |
| `getPackages` | 查询小程序资源列表 |
| `getNextVersion` | 拉取全部资源分页并计算下一版本 |
| `getUploadDefaults` | 查询上一版详情和应用信息，预览可继承默认值 |
| `getResource` | 根据 `packageId` 查询资源 |
| `createTask` | 为已有 `packageId` 创建任务 |
| `getTaskList` | 查询某个资源的任务列表 |
| `getTaskDetail` | 根据 `taskId` 查询任务详情 |
| `changeTaskStatus` | 为 `packageId`、`taskId` 修改任务状态 |

单独上传资源目录或已有 ZIP 时，入口和扩展参数默认继承上一版；完整工具检测到源码目录后，会先编译并从编译元数据读取 `mainUrl`、`extendInfo`，再打包上传。已经编译的目录会直接封装为 `<小程序ID>.zip`，ZIP 内部以小程序 ID 为根目录；已有 ZIP 继续兼容。独立 `mpaas-openapi-rs` 只负责 OpenAPI 调用，没有 minidev 编译器，使用它时目录必须已经编译；源码目录会被拒绝。`release` 命令自动传递本次编译元数据，保留上一版其他默认信息。

`upload` 与 `uploadAndPublish` 都自动查询后台版本。默认省略 `upload.h5Version`；填写时它是期望值，必须与自动计算结果一致。`release` 会在编译前查询、上传前复查，若版本已变化则停止上传。未知资源列表结构、四段版本或末位不在 0–19 的历史版本会报错，不推测新版本。完整规则及配置示例见 [自动上传与发布](docs/publishing.md)。

SDK 库入口为 `run(&Config)`；`MpaasClient` 提供各接口方法。运行结果 JSON 写入 stdout，默认 info 日志写入 stderr，日志不输出私钥或签名 URL。连接与请求设有超时，禁止自动重定向。旧 `scripts/verify-mpaas-openapi.mjs` 和 `config/mpaas-*.example.json` 保留为历史转换/抓包验证材料，新发布流程与 `npm run verify:openapi` 均使用 Rust 客户端。

Rust SDK 为 PDF 和 Java Demo 的每个接口提供有名称的方法，以及对应的 serde 请求和响应类型。CLI 配置使用 `action: "api"`、`api.path` 和 `api.params`，请求方式由 SDK 确定。模板见 [API 配置示例](config/api.example.json)。

## 验证

```bash
npm test
cargo test --locked --manifest-path rust-sdk/Cargo.toml
cargo fmt --check --manifest-path rust-sdk/Cargo.toml
npm run verify:openapi-demo -- rust-sdk/target/release/mpaas-openapi-rs
npm run verify:deploy -- rust-sdk/target/release/mpaas-openapi-rs --client
npm run package:sea
node scripts/verify-release.mjs dist/mpaas-amr-<platform>-<arch>
```

`verify:openapi-demo` 仅检查独立 Rust 客户端的只读查询、签名和错误识别。`verify:deploy --client` 验证自动版本、上传、发布和恢复。最后一个命令还会实际编译项目，并验证创建任务失败时保留 ID、编译后版本变化时阻断上传。它们均使用本机模拟服务和临时 RSA 密钥，不请求真实后台。平台 CI 还检查 Linux 无网络、无 Node 的容器和 Windows 隔离 PATH 下的编译，以及内置 Rust 客户端能否执行。

现有归档与参考样包的核查结果见 [包兼容性报告](docs/package-compatibility.md)。当前确认上传 ZIP 的命名、目录和内容保留满足文档要求；AppxNG/web 与目标服务端、客户端 SDK 的兼容性仍需真实环境验证。
