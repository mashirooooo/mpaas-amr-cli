# 使用说明

## 交付文件

完整工具按平台提供单文件，内置 Node 运行时、minidev 编译器及 Rust OpenAPI 客户端；运行机器无需安装这些开发工具。首次运行会向系统临时目录释放内置资源，需要该目录可写且允许执行。

| 平台 | 完整工具 | 当前验证情况 |
| --- | --- | --- |
| Linux x64 / glibc | `mpaas-amr-linux-x64` | Debian 12 干净容器编译、包校验和本机模拟后台验证通过 |
| macOS arm64 | `mpaas-amr-darwin-arm64` | 本机编译、包校验和本机模拟后台验证通过 |
| Windows x64 | `mpaas-amr-win32-x64.exe` | 已配置 CI，尚未完成 Windows 运行验证 |

下文以 Linux 为例；macOS 替换可执行文件名，Windows 使用对应 `.exe`。命令中的项目、配置和输出路径均按实际位置填写。

## 查询 OpenAPI

使用 [OpenAPI demo](../demo/openapi/README.md) 及其 [配置模板](../demo/openapi/config.example.json)。当前无需准备项目、图标或资源包，只需真实后台参数、已创建的小程序 ID 和与后台公钥配对的私钥。

```bash
chmod +x ./mpaas-amr-linux-x64
./mpaas-amr-linux-x64 openapi ./config.local.json
```

模板保持 `action: "getPackages"`。结果 `ok: true` 且退出码为 `0` 表示本次查询调用成功，资源列表为空也可以通过。标准输出为 JSON，日志写入标准错误。

若只测试 OpenAPI，可以使用更小的独立 Rust demo 包：

```bash
./mpaas-openapi-rs ./config.local.json
```

两种方式调用相同的 Rust 客户端。该只读查询不验证上传和发布权限。

用户已确认通过的查询 demo 已[归档](../archive/openapi-demo-2026-11-17/README.md)。自动上传、发布和恢复操作见 [自动上传与发布](publishing.md)，对应生产包为 `mpaas-publisher-<平台>-<架构>.zip`。

Windows 可直接下载独立的 `mpaas-openapi-demo-win32-x64.zip`，解压并填写同一份配置，然后在 PowerShell 中执行：

```powershell
.\mpaas-openapi-rs.exe .\config.local.json
$LASTEXITCODE
```

此包仅包含 OpenAPI 客户端，使用时不需要完整编译工具。完整 Windows 工具的构建与验证状态仍见上表。

## 编译和打包

```bash
./mpaas-amr-linux-x64 build \
  --project /path/to/mini-project \
  --app-id 0000000000000012 \
  --version 1.0.0.1 \
  --output /path/to/artifacts \
  --minify
```

默认生成两种归档及构建元数据：

```text
artifacts/1.0.0.1/
├── 0000000000000012.zip
├── 0000000000000012.amr
├── 0000000000000012.tar
└── 0000000000000012.build.json
```

`--app-id` 是小程序 ID，`--version` 使用待上传的新版本号。可加 `--format zip` 或 `--format amr` 只生成一种归档；`--parallel`、`--typescript`、`--less` 按项目需要启用。

上传接口最终使用 ZIP。完整工具的 `upload.resource` 可以填写源码目录、未打包的编译资源目录或已有的合规 ZIP；检测到源码目录时会先编译，再从编译产物生成 `<h5Id>.zip`，已经编译的目录则直接封装，临时包在上传完成后清理。独立 Rust Publisher 没有编译器，源码目录会被拒绝。AMR 为兼容产物，不能将本地未签名 AMR 作为平台已签名的正式分发包。

## 校验资源包

```bash
./mpaas-amr-linux-x64 validate \
  /path/to/artifacts/1.0.0.1/0000000000000012.zip \
  --app-id 0000000000000012
```

同一命令也支持 `.amr`。校验用于确认归档结构和内容约束；不能代替服务端处理或真机启动验收。AMR 校验结果中的 `signatureVerified: false` 表示未验证平台签名。

## 后续上传发布

复制 [发布配置模板](../config/release.example.json) 为 `release.local.json`，填写主应用、工作空间、租户、私钥、构建项目和本地图标。业务字段默认继承上一版资源和小程序应用信息，首次上传需补足仍缺失的必填项。这里的 `appId` 是 mPaaS 主应用 ID，`build.appId` 是小程序 ID。

所有配置内相对路径以配置文件目录为基准。入口 `mainUrl` 和启动参数由编译结果传递，无需照抄样例首页。

先只读查询下一版本，再编译、校验，不上传或发布：

```bash
./mpaas-amr-linux-x64 release ./release.local.json --prepare-only
```

编译、上传并启动发布任务：

```bash
./mpaas-amr-linux-x64 release ./release.local.json
```

该命令会写入后台。`publishType: 3` 表示正式发布；使用前核对配置中的目标环境、版本和发布范围。

`release` 的版本来自后台资源列表，省略配置中的 `build.version`。首次上传为 `0.0.1`，末位到 20 时向中间位进一，例如 `0.0.19 → 0.1.0`；首位保持不变。上传前会再次检查版本，避免将按旧版本编译的产物直接上传为另一版本。

成功结果包含 `packageId`、`taskId` 及 `taskStatus: 1`，表示后台任务进入“发布中”。部分失败会保留已经取得的 ID 和失败阶段；先查询已有资源和任务，再决定恢复操作，避免直接重复创建。

完整配置、单接口 action 和源码构建方式见 [README](../README.md)。包格式及真实环境验收边界见 [兼容性报告](package-compatibility.md)。
