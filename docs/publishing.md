# 自动上传与发布

已验证的只读 demo 基线已归档，源码工程中的归档目录为 `archive/openapi-demo-2026-11-17`。正式功能使用下列上传、发布和编译发布入口；demo 归档保持原样，不随正式功能更新。

## 版本自动分配

每次上传前，客户端按小程序 ID 和资源包类型查询资源列表的全部分页，按三段数字比较取最新版本，再计算下一版：

| 后台最新版本 | 本次上传版本 |
| --- | --- |
| 无资源 | `0.0.1` |
| `0.0.9` | `0.0.10` |
| `0.0.18` | `0.0.19` |
| `0.0.19` | `0.1.0` |
| `0.19.19` | `0.20.0` |
| `1.0.19` | `1.1.0` |

末位范围为 0–19；达到 20 时归零并给中间位加一。首位保留后台最新版本的值，不参与进位。版本按数字比较，不能按字符串排序。

上传配置默认省略 `upload.h5Version`，由客户端填写。若填写，它作为本次上传的期望版本；与查询结果不一致时停止上传。历史资源不是三段数字版本、末位超出范围，或者后台列表结构无法识别时，会明确报错，不猜测新版本。

## 上传信息默认继承

客户端选取最新数字版本对应的资源，查询其详情，并用上一版信息填充本次省略的上传字段。名称或虚拟域名仍缺失时，按小程序 ID 查询应用信息补充。

优先级为：本次显式配置 → 上一版资源 → 小程序应用信息 → 内置默认值。显式的 `0` 和可选字段的 `null` 会保留，不会被旧值覆盖。

| 可省略的配置 | 默认来源 |
| --- | --- |
| `h5Name` | 上一版名称，缺失时使用小程序应用名称 |
| `vhost` | 小程序应用的虚拟域名 |
| `clientVersionMin` / `clientVersionMax` | 上一版客户端版本范围 |
| `platform` | 上一版平台，最终缺省为 `all` |
| `autoInstall` / `installType` | 上一版安装设置 |
| `enableTabBar` / `enableOptionMenu` / `enableKeepAlive` | 上一版运行开关 |
| `mainUrl` / `suburl` | 上一版入口，次入口也可从应用信息补充 |
| `extendInfo` | 上一版扩展信息；本次提供的字段及 `launchParams` 字段覆盖对应旧值 |

使用完整 `mpaas-amr` 工具时，`upload` 可以直接填写 minidev 源码目录：

```json
{
  "resource": "C:/packages/0000000000000012",
  "icon": "C:/packages/icon.png",
  "h5Id": "0000000000000012"
}
```

如果目录包含 `mini.project.json`、`.axml`、`.acss` 或 `.less`，完整工具会先调用内置 minidev 编译器，使用编译生成的 TAR 创建 ZIP，再上传 ZIP；本次编译得到的入口和启动参数会随上传配置传递。已经编译但尚未压缩的目录也可填写，客户端会校验目录内容并临时生成 `<h5Id>.zip`，ZIP 内部使用 `<h5Id>/` 作为根目录，上传完成后自动清理临时文件。已有符合要求的 `<h5Id>.zip` 仍可直接填写，客户端继续兼容。

独立 `mpaas-openapi-rs` 和 Windows `mpaas-publisher` 只包含 Rust OpenAPI 客户端，不包含 minidev 编译器。使用这两个程序时，`upload.resource` 必须是已经编译的目录或 `<h5Id>.zip`；检测到源码目录会直接报错。需要从源码完成编译、上传和发布时，请使用完整的 `mpaas-amr-<platform>-<arch>` 单文件工具。

这是完整配置中的 `upload` 片段；后台地址、鉴权参数和发布设置仍使用对应模板。图标是本次 multipart 上传的本地文件，需要自行提供。

运行时，结构化结果仍写入标准输出，过程日志写入标准错误。日志会显示当前阶段、请求方法和接口路径、查询或表单参数、响应状态及响应正文，签名、私钥和凭据字段会脱敏。需要调整日志级别时可设置 `RUST_LOG`，例如 `RUST_LOG=debug`。

首次上传没有上一版可继承，客户端仍会尝试从应用信息补充名称和虚拟域名；`clientVersionMin` 等剩余必填字段需要显式填写。缺少必填项时会在上传前提示。需要修改旧配置时，只在 `upload` 中增加对应字段即可。

如果应用列表没有返回虚拟域名，客户端会继续查询 `getNebulaVhost`；如果仍然缺少必填字段，错误会一次性列出所有缺失字段。

只读预览下一版本和可继承默认值：复制 `upload-defaults.example.json`，填写鉴权和小程序 ID，运行：

```powershell
.\mpaas-openapi-rs.exe .\upload-defaults.local.json
```

对应 `action: "getUploadDefaults"`，返回 `data.latestVersion`、`data.nextVersion` 和 `data.uploadDefaults`，不上传文件或创建任务。

## Windows 独立上传发布包

下载 `mpaas-publisher-win32-x64.zip` 并解压。包内包含 `mpaas-openapi-rs.exe`、本文和六个配置模板；不需要 Node、Rust、Java 或 minidev。目标系统为 Windows 10/11 或 Windows Server 2016 及以上的 x64 系统。

此包接收已经编译好的小程序资源目录或 ZIP，支持自动打包、上传、自动发布和发布恢复。源码编译使用下一节的完整工具。

本轮 Windows 生产程序已完成交叉编译及 PE/系统 DLL 检查，新增上传发布流程尚未在 Windows 实机执行；对应 Rust 流程已在 macOS、Linux 的本机模拟后台验证。

在 PowerShell 中复制配置并编辑：

```powershell
Copy-Item .\upload-and-publish.example.json .\upload-and-publish.local.json
notepad .\upload-and-publish.local.json
```

需要填写：

- `baseUrl`：OpenAPI 服务 origin，只有协议、主机和端口，不追加 `/openapi`。
- `appId` / `workspaceId` / `tenantId`：mPaaS 主应用、工作空间和租户。
- `privateKeyFile`：与后台公钥配对的 RSA 2048 私钥文件路径。
- `upload.h5Id`：16 位小程序 ID，与根级 `appId` 不同。
- `upload.resource`：Windows 独立 Publisher 必须填写已经编译的资源目录或符合要求的 ZIP；完整工具才可以填写源码目录并在上传前编译。已经编译的目录会生成以小程序 ID 为文件名、以小程序 ID 为根目录的 ZIP。
- `upload.icon`：目标应用的本地图标文件。
- `vhost` / `clientVersionMin` 等业务字段：已有版本时默认继承；需要覆盖或首次上传仍有缺项时再填写。
- `upload.mainUrl` / `extendInfo`：已有版本默认继承；本次编译入口或启动参数改变时，从实际生成的 `<小程序ID>.build.json` 读取并覆盖，保留其中 `extendInfo.launchParams`。
- `publish`：默认 `publishType: 3`，表示正式发布；正式发布不填 `publishMode`。

相对路径以配置文件目录为基准，Windows 绝对路径可以写成 `C:/packages/0000000000000012.zip`。

上传并自动创建、启动发布任务：

```powershell
.\mpaas-openapi-rs.exe .\upload-and-publish.local.json
$LASTEXITCODE
```

只上传时复制 `upload.example.json`，保持 `action: "upload"`，然后用同样命令运行对应配置。只查询下一版本时使用 `version.example.json`（`action: "getNextVersion"`），该查询不会占用版本号。

Linux/macOS 生产包中的程序名为 `mpaas-openapi-rs`，运行方式为 `./mpaas-openapi-rs 配置路径`。

通过 CLI 调用其他模块时，复制包内的 `api.example.json`，在 `api.path` 填写接口路径，并在 `api.params` 填写请求模型对应的字段。字段含义、必填条件和响应类型见 [接口目录](api-reference.md)。

## 从源码自动编译、上传并发布

完整工具使用源码工程中的 `config/release.example.json` 模板。不需要填写固定的 `build.version`；程序先查询下一版本，再用这个版本编译，自动传递入口和启动参数。上一版的其他业务默认值继续继承；编译器计算出的本次 `mainUrl` 和启动参数优先于历史值。名称仅在 `upload.h5Name` 或 `build.name` 显式指定时覆盖后台默认名称。

```bash
./mpaas-amr-linux-x64 release ./release.local.json
```

流程如下：

```text
查询最新版本 → 计算下一版 → 编译并校验 ZIP
                                  ↓
再次查询版本，确认未变化 → 上传 → 创建任务 → 查询状态
                                                ↓
                                  待发布则启动 → 再次确认发布中
```

两次版本查询之间若已有其他人上传，程序会在写入前拒绝本次发布，不把编译结果改名后冒充新版本。后台仍需校验并发请求的版本唯一性；客户端不自动重试上传和发布写请求。

仅准备编译产物时执行：

```bash
./mpaas-amr-linux-x64 release ./release.local.json --prepare-only
```

此命令会只读查询后台以确定版本，随后编译校验，不上传、不发布。完全离线编译使用 `build --version <版本>`。

源码目录、已有资源目录或 ZIP 时，完整工具也提供独立入口；源码目录会先编译：

```bash
./mpaas-amr-linux-x64 upload ./upload.local.json
./mpaas-amr-linux-x64 upload ./upload.local.json --publish
./mpaas-amr-linux-x64 publish ./publish.local.json
```

macOS 使用对应的 `mpaas-amr-darwin-arm64`。Windows 完整编译工具仍由 Windows 原生 CI 构建验证；当前独立 Windows 发布包可以先使用已有 ZIP 完成上传发布。

## 发布已有资源与恢复

复制 `publish.example.json`，设置 `packageId` 为已有资源 ID；保持 `action: "publish"`。首次执行会创建发布任务、按需启动并确认状态。

如果之前已经取得 `taskId`，在同一配置根级增加它：

```json
{
  "packageId": "101",
  "taskId": "202"
}
```

这是需要合并到完整配置中的 ID 片段。此时客户端直接查询已有任务，不重复上传或创建任务；待发布任务会启动，已发布中的任务只查询确认。任务资源 ID 不匹配或处于其他状态时明确报错。

## 输出与失败处理

结果 JSON 写入标准输出，日志写入标准错误。成功退出码为 `0`，失败为 `1`。上传发布成功结果包含 `h5Version`、`packageId`、`taskId` 和 `taskStatus: 1`。

`taskStatus: 1` 表示后台任务进入“发布中”；客户端安装和页面启动仍需目标环境验收。部分失败会保留已经取得的 ID、版本和失败阶段。上传成功后若创建或启动任务失败，应使用这些 ID 运行 `publish` 恢复，避免重新执行上传。

本轮开发使用临时 RSA 密钥与本机模拟后台验证自动版本、上传发布、错误阻断和恢复；没有请求真实后台。用户已确认通过的是归档中的 OpenAPI 查询 demo，正式上传发布需在目标环境验收。
