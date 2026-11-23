# OpenAPI 只读验证 demo

本 demo 调用 `GET /openapi/miniprogram/getPackages` 查询已有小程序的资源列表，验证网络连接、RSA 签名、查询权限和响应解析。不编译项目，不上传资源，不创建或启动发布任务。

## 1. 准备文件

下载对应平台的 `mpaas-openapi-demo-<平台>-<架构>.zip`，解压后包含：

```text
mpaas-openapi-demo-<平台>-<架构>/
├── mpaas-openapi-rs        独立 Rust 客户端；Windows 为 mpaas-openapi-rs.exe
├── config.example.json    只读查询模板
└── README.md
```

提供 Linux x64、macOS arm64 和 Windows x64 包。Linux 包已在 Debian 12 验证，使用 glibc；Alpine/musl 不能直接运行该包。无需安装 Node、Java、Rust 或 OpenSSL 命令行。

Windows 包面向 Windows 10/11 或 Windows Server 2016 及以上的 x64 系统。本次包由交叉编译生成，已检查 PE 架构和 DLL 依赖，只导入 Windows 自带 DLL；Windows 实机尚未验证。

本机 Wine 8 缺少该 Rust 版本所需的 `bcryptprimitives.dll`，在加载阶段退出，因此本轮未完成 Windows 运行验证。无需为 Windows 客户端额外下载此 DLL；它属于目标 Windows 系统组件。

也可以直接使用完整工具的 `openapi` 子命令，见第 5 节。

## 2. 填写配置

复制 `config.example.json` 为同目录下的 `config.local.json`，修改以下字段：

| 字段 | 填写内容 |
| --- | --- |
| `action` | 保持 `getPackages`，仅执行查询 |
| `baseUrl` | mcube OpenAPI 服务地址，例如 `https://mcube.example.internal:8281`；仅协议、主机和端口，不追加 `/openapi` 或控制台路径 |
| `appId` | mPaaS 主应用 ID，与小程序 ID 不同 |
| `workspaceId` | 目标工作空间，例如 `dev`；使用后台实际值 |
| `tenantId` | 目标租户 ID；`ANTCLOUD` 只是示例，使用后台实际值 |
| `privateKeyFile` | 与后台已配置公钥配对的 RSA 2048 私钥文件路径 |
| `h5Id` | 后台已创建的小程序 ID，保留前导零 |
| `packageTypes` | `"1"`，查询正式功能包 |
| `pageNum` / `pageSize` | 页码和页大小，保持 `1` / `10` 即可；如省略则同时省略 |
| `timeoutSeconds` | 请求超时秒数，默认示例为 `60` |

将现有私钥放到 `keys/private-key.pem`，或者修改 `privateKeyFile` 指向现有文件。后台必须已经配置与该私钥配对的公钥；临时生成一个新私钥不能直接通过真实后台鉴权。

私钥支持无密码 PKCS#8 / PKCS#1 PEM，以及裸 Base64 PKCS#8。相对路径以配置文件所在目录解析。Windows 绝对路径建议写为 `C:/keys/private-key.pem`，或者在 JSON 中将反斜杠写为 `\\`。

若目标部署要求 Java 示例中的 `loginName` Cookie，可在配置中添加 `"loginName": "实际用户标识"`；默认不发送该 Cookie。

## 3. 执行查询

Windows：在解压目录打开 PowerShell。先复制并编辑配置，放好与后台公钥配对的私钥：

```powershell
Copy-Item .\config.example.json .\config.local.json
notepad .\config.local.json
.\mpaas-openapi-rs.exe .\config.local.json
$LASTEXITCODE
```

`$LASTEXITCODE` 打印查询命令的退出码。需要保存结果和日志时执行：

```powershell
.\mpaas-openapi-rs.exe .\config.local.json > result.json 2> error.log
$LASTEXITCODE
```

Linux / macOS：在解压目录打开终端，复制并编辑配置、放好私钥后执行：

```bash
chmod +x ./mpaas-openapi-rs
./mpaas-openapi-rs ./config.local.json
echo $?
```

最后一行打印上一条查询命令的退出码。需要保存结果和日志时执行：

```bash
./mpaas-openapi-rs ./config.local.json > result.json 2> error.log
echo $?
```

## 4. 判断结果

成功时退出码为 `0`，标准输出示例：

```json
{
  "ok": true,
  "action": "getPackages",
  "data": []
}
```

`data` 是后台 `data.data` 的原值，可能是数组或分页对象。空列表也表示查询调用成功，不代表已经存在资源包；有数据时继续核对是否为预期小程序的资源。

失败时退出码为 `1`，结果中 `ok` 为 `false`，并包含 `error.kind` 和 `error.message`。日志写到标准错误，不影响标准输出 JSON。

| `error.kind` | 检查方向 |
| --- | --- |
| `config` | 配置 JSON、必填字段、`baseUrl` 格式、分页参数 |
| `key` | 私钥路径、读取权限、格式及 RSA 2048 位要求 |
| `http` | DNS、端口、网络连通、超时、TLS 证书 |
| `httpStatus` | 非 2xx 状态码；包括重定向，检查服务地址及代理路由 |
| `response` | 返回 HTML 登录页、非 JSON 或不符合 MDS 响应封装 |
| `api` | 查看后台错误码及消息，核对公钥、主应用、工作空间、租户、查询权限和本机时间 |

本 demo 不需要浏览器登录会话。返回登录页面通常表示访问地址或网关鉴权不符合 OpenAPI 调用要求。

查询成功验证的是此接口的连接、签名与查询权限；上传和发布权限需要后续分别验证。

## 5. 使用完整工具验证

已有完整单文件工具时，无需再下载独立 Rust 客户端，同一份配置可以直接使用：

```bash
# Linux
./mpaas-amr-linux-x64 openapi ./config.local.json

# macOS Apple Silicon
./mpaas-amr-darwin-arm64 openapi ./config.local.json
```

Windows 完整工具构建完成后，在 PowerShell 中执行：

```powershell
.\mpaas-amr-win32-x64.exe openapi .\config.local.json
$LASTEXITCODE
```

以上命令中的工具和配置路径按实际位置修改。Windows 独立 OpenAPI demo 与包含编译器的完整工具是两个交付文件；当前先提供独立 OpenAPI demo，完整 Windows 工具仍需 Windows CI 构建验证。

## 开发者本地模拟验证

在源码工程中，开发者可运行以下命令验证客户端，使用临时生成的密钥和本机模拟服务，不连接真实后台：

```bash
node scripts/verify-openapi.mjs rust-sdk/target/release/mpaas-openapi-rs
```

此开发验证检查 GET 路径、RSA 签名、查询参数、成功空列表、业务失败和 HTTP 200 登录页。它只调用 OpenAPI，不执行编译、上传或发布；执行该开发脚本需要 Node，最终用户运行上述 demo 不需要。

重新生成交付包时，指定对应平台的原生客户端；例如在 Windows 构建后执行：

```powershell
npm run build:openapi
npm run verify:openapi-demo -- rust-sdk/target/release/mpaas-openapi-rs.exe
npm run package:openapi-demo -- rust-sdk/target/release/mpaas-openapi-rs.exe win32-x64
```

ZIP 和对应 `.sha256` 文件写入 `dist`。这些是构建机器的开发命令，最终用户无需执行。
