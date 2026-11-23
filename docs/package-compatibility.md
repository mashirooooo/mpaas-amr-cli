# 小程序资源包兼容性核查

核查日期：2026-11-17。本文核查现存 `artifacts/1.0.0.0` 产物、参考工程中的 ZIP 样包及已提取的 PDF 文本；没有修改参考文件、读取密钥或本地凭据，也没有请求目标服务器。

## 结论与验收边界

- **已验证：**当前 `0000000000000012.zip` 是以小程序 ID 为目录前缀的 ZIP，名称、目录前缀与上传文档要求一致。其 5 个业务文件与编译 TAR 中对应文件逐字节相同；AMR 内部 TAR 与 `build/dist.tar` 也逐字节相同。
- **已验证：**当前 AMR 的顶层是 TAR、`Manifest.xml` 和 `CERT.json`，不符合 `resourceFile` 要求的应用 ID 目录结构，不能作为该接口的上传资源直接发送。更改扩展名不会改变此结果。应上传已经展开业务文件的 ZIP。
- **已验证：**当前内容是 minidev 生成的 AppxNG/web 小程序编译资源；参考 ZIP 是 `www/` 下的静态网页资源，两者布局和运行依赖不同。
- **待目标环境验收：**文档与样包没有给出足以判定 AppxNG/web 编译内容兼容性的完整内部格式规范。当前只能确认外层归档和内容保留正确，不能据此宣称上传、平台处理、发布和客户端启动均已兼容。

## 核查来源

| 来源 | 本地路径 | 用途 |
| --- | --- | --- |
| 公共协议与变更记录 | `/Volumes/V1/work/WXWorkLocal/chat/file/202611/MDS 私有云 openApi/api文档-0-概述.pdf` | 公共参数、RSA 鉴权、业务返回结构、版本新增字段 |
| 小程序上传接口 | `/Volumes/V1/work/WXWorkLocal/chat/file/202611/MDS 私有云 openApi/小程序/mds小程序api文档-2-上传小程序资源包.pdf` | ZIP 命名、目录、上传参数及上传结果 |
| 概述提取文本 | `/private/tmp/mpaas-openapi-reference/api文档-0-概述.txt` | 按 `PAGE` 标记核对概述 PDF 页码 |
| 上传提取文本 | `/private/tmp/mpaas-openapi-reference/mds小程序api文档-2-上传小程序资源包.txt` | 按 `PAGE` 标记核对上传 PDF 页码 |
| 参考 ZIP | `/Volumes/V1/work/WXWorkLocal/chat/file/202611/MDS 私有云 openApi/demo/src/main/resource/miniprogram/1111111111111111.zip` | 只读列举归档及读取资源内容 |
| 参考调用代码 | `/Volumes/V1/work/WXWorkLocal/chat/file/202611/MDS 私有云 openApi/demo/src/main/java/openapi/mds/miniprogram/MiniProgramApi.java` | `uploadResource()`，第 110–130 行 |
| 当前工程 | `/Volumes/V1/work/minidevTools/mpaas-amr-cli` | 下文工程相对路径均以此为根目录 |

## PDF 能够证明的要求

上传 PDF 第 1–3 页规定 `POST /openapi/miniprogram/uploadPackage`，`resourceFile` 必须是 ZIP，ZIP 文件名和解压后的文件夹名称均为小程序 App ID；`resourceType` 固定为 `4`。`mainUrl` 是可选的小程序包主入口，`iconFile` 是必传的 JPG/PNG 图标。

文档没有在这些页面规定业务包必须包含 `www/`、`index.js`、`appConfig.json` 或某一类 manifest，也没有将这些文件的存在或缺失作为兼容性判据。因此不能因当前 ZIP 比参考样包多出 `index.js` 等文件，就判断当前编译结果不兼容。

概述 PDF 第 4–5 页同时要求公共参数 `tenantId`、`workspaceId`、`appId`，以及 URL 上的 `appId`、`workspaceId`、秒级 `timestamp` 和 RSA 2048 签名 `sign`。完整返回结构中，业务成功标志位于 `data.success`，接口返回值位于 `data.data`。上传 PDF 第 3 页的 `"189"` 是接口数据示例，不代表完整 HTTP 返回体只有一个字符串。归档格式核查通过不等于这些请求与响应要求已经满足。

## 现存归档结构对比

参考 ZIP 的主要内容为：

```text
1111111111111111.zip
├── 1111111111111111/
│   ├── .DS_Store
│   └── www/
│       ├── 12345678_4_5_6_7.html
│       ├── css/top.css
│       ├── images/…多个 HTML 及 loading.gif
│       └── js/user.js
└── __MACOSX/…/._loading.gif
```

参考 ZIP 共 133 个条目、128 个文件；其中有 121 个 HTML、1 个 CSS、1 个 JS、1 个实际 GIF，以及 3 个 `.DS_Store` 和 1 个 AppleDouble 元数据文件。它没有 `index.html`、`main.html`、`app.json`、`appConfig.json`、`manifest.json`、`Manifest.xml` 或 `CERT.json`。示例 HTML 使用常规页面标签和相对 JS/CSS 引用。

参考样包还包含应用 ID 目录之外的 `__MACOSX` 条目。这是可确认的样包杂项，不能据此推导生产上传必须保留它，也不能证明目标服务接受任意额外顶层目录。当前 ZIP 没有这些系统元数据。

当前 ZIP 与当前构建 TAR 的内容为：

```text
0000000000000012.zip           build/dist.tar
└── 0000000000000012/          ├── appConfig.json
    ├── appConfig.json        ├── index.html
    ├── index.html            ├── index.js
    ├── index.js              ├── index.worker.js
    ├── index.worker.js       └── manifest.json
    └── manifest.json
```

ZIP 不需要显式的空目录条目才能表达上述目录；现存 ZIP 的 5 个文件名均以 `0000000000000012/` 为前缀。TAR 中业务文件直接位于根目录，不能将整个 TAR 当作 ZIP 内唯一的业务文件上传。

当前 AMR 的内容为：

```text
0000000000000012.amr
├── 0000000000000012.tar
├── Manifest.xml
└── CERT.json
```

`Manifest.xml` 记录 UID `0000000000000012`、名称 `h5app`、版本 `1.0.0.0`；`CERT.json` 对 TAR 和 XML 的签名值均为空。此 AMR 是工程自身封装的结构产物，不能作为已经由平台签名的正式分发包。上传 PDF 第 4 页的 `downloadUrl` 示例指向服务端产出的 `.amr`，这也不能反推该接口接受客户端直接上传 AMR。

## 编译内容、manifest 与版本差异

当前安装的 minidev 包版本为 `2.2.5`。现存 `build/mini.built.json` 明确记录：

```json
{
  "compileVersion": "ng",
  "renderType": "web",
  "enableSubPackagesOnlyBuild": false,
  "packageInfo": {
    "appType": "tinyApp",
    "mainUrl": "/index.html#pages/index/index"
  }
}
```

`build/ng-build-stat.json` 还记录 `appxRuntimeRequired: "26"`、`cubeRuntimeRequired: "93"` 和 `subpackmode=whole`。这些是编译器报告的要求与构建参数；现有 PDF 没有将这些内部数值映射到 mPaaS 客户端 SDK/服务端版本，不能据此直接认定目标 SDK 满足要求，也不能仅因报告中出现 `cubeRuntimeRequired` 就将资源改为 `cubeResourceFile` 上传。

当前 `index.html` 引用 `https://appx/af-appx.min.css`、`https://appx/af-appx.min.js`、`index.js`，并通过 `bootstrapApp()` 加载 `index.worker.js`，代码中有 `AppxNG` 标记。这些内容说明它依赖相应的小程序运行环境，单纯在普通浏览器中打开成功与否也不足以判断 mPaaS 容器兼容性。

| 项目 | 参考 ZIP | 当前 ZIP |
| --- | --- | --- |
| 可见内容形态 | `www/` 下的静态网页与资源 | AppxNG/web 编译资源 |
| 编译版本证据 | 没有可识别的 minidev 版本或构建元数据 | 本地依赖为 minidev 2.2.5；构建元数据为 `ng`/`web` |
| 时间线索 | ZIP 顶层目录记录时间为 2018-08-15；Java 示例注释日期为 2020-04-26 | 本次核查的是现存 `1.0.0.0` 产物 |
| `appConfig.json` | 无 | 包含 `pages`、`window`、`launchParams`、`prerenderPage: "index.html"` |
| `manifest.json` | 无 | 仅含 `gcm_sender_id`、`gcm_user_visible_only`，是 HTML 引用的 Web manifest |
| `Manifest.xml` | 无 | ZIP 中无；仅在工程封装的 AMR 外层存在，和小写 `manifest.json` 用途不同 |

参考归档时间不等于其对应的 mPaaS 版本，也不能证明该样包使用了某一版小程序编译器。它不能作为当前目标环境已经通过启动验收的标准包。尤其是 Java 示例第 116 行填写 `mainUrl=/main.html`，而样包中不存在 `main.html`；不能直接复用这个入口值。

## 主入口应来自 app.json

当前源工程 `fixtures/minimal/app.json` 的首个页面为 `pages/index/index`，编译后的 `appConfig.json.pages[0]` 与 `mini.built.json` 中的入口一致，因此当前样例的 `/index.html#pages/index/index` 有实际来源。

对其他工程，应读取项目实际 `app.json`（考虑项目配置中的小程序根目录），以首个页面确定默认启动页，并核对编译后的应用配置。应优先使用编译器已经计算出的 `mini.built.json.packageInfo.mainUrl`，交叉检查它与实际页面的对应关系，避免重新猜测不同编译类型的入口。当前 web 样例的结果是 `/index.html#<首个页面>`，不能把 `pages/index/index` 固定用于所有工程。若显式配置 `mainUrl`，应校验它与编译结果一致；没有有效入口时应明确报错。Cube 或其他编译类型的入口约定仍需对应版本文档确认。

## 编译元数据的传递

现存 `build/mini.built.json.packageInfo` 不仅提供 `mainUrl`，还通过 JSON 字符串 `extendInfo` 提供以下启动参数：

```json
{
  "launchParams": {
    "enableTabBar": "NO",
    "enableJSC": "YES",
    "page": "pages/index/index",
    "enableKeepAlive": "YES",
    "enableWK": "YES",
    "appxRouteFramework": "YES",
    "appxRouteBizPrefix": ""
  }
}
```

原 `scripts/verify-mpaas-openapi.mjs` 上传流程默认使用空对象 `extendInfo={}`，并硬编码 `/index.html#pages/index/index`。直接沿用这两个默认值，可能丢失编译器指定的启动参数，或在首个页面不同的工程中选择错误入口。现有文档不足以判定每个启动参数在目标 SDK 中的必要性，因此这里识别的是元数据丢失风险，不宣称某个单独字段必然造成启动失败。

新 `src/build.mjs` 读取编译器 `packageInfo.mainUrl` 和 `packageInfo.extendInfo`，随产物写入 `<appId>.build.json`。新 `src/release.mjs` 在准备上传配置时使用该入口并传递 `extendInfo.launchParams`，合并调用方额外扩展字段；与编译结果冲突的小程序 ID、版本和入口会被拒绝。这避免通过文件名猜测启动配置，也避免默认空对象覆盖编译信息。新增测试验证了这条元数据传递路径，尚未因此宣称目标客户端已兼容。

## cubeResourceFile 与 subpackageFlag 的证据范围

概述 PDF 第 3 页的变更记录提供了以下信息：

| 字段 | 引入版本 | 文档直接说明 | 尚不能由此证明 |
| --- | --- | --- | --- |
| `subpackageFlag` | 2.51.1 | 表示当前上传 ZIP 资源是否支持分包 | 字段数据类型、取值、默认值、内部子包布局及缺省行为；不能充当任意编译格式转换开关 |
| `cubeResourceFile` | 2.51.4 | 表示要上传的 Cube 小程序资源文件 | Cube 包内部结构、扩展名、与 `resourceFile` 的组合/替代规则、是否必传，以及当前 NG/web 包是否应使用此字段 |

所给上传 PDF 的参数表和 Java 示例未补充这两个字段的完整定义。当前样例没有分包声明，构建参数为 `whole`；没有证据要求它使用分包上传。不能为了“兼容”盲目添加字段、修改 ZIP 内部布局，或将 web 产物冒充 Cube 资源。应先取得目标版本接口定义或由同版本官方工具导出的对应产物，再配置这些能力。

## 逐字节比对证据

以下是核查时现存文件的大小和 SHA-256；若重建覆盖 `artifacts/1.0.0.0`，应重新计算这些值。

| 归档 | 字节数 | SHA-256 |
| --- | ---: | --- |
| 参考 `1111111111111111.zip` | 719718 | `f1be228a1833466071c3ad91bbf3bbe9a259e9fc1374ef0ec3516e1e31a9c649` |
| 当前 `0000000000000012.zip` | 12070 | `1419f4c1be72556f8422d87709e65e27af975b83ec6d367cbebac555282e8d8e` |
| 当前 `0000000000000012.amr` | 10676 | `98480d164e253cf40b3c9ebaef2a1d81e89c70d012d313e91e4fb7fb0fa1c063` |
| 当前 `build/dist.tar` | 47616 | `bb2ae0cea3ab771be1c2b7ea467073bc9866d11ecaeb66fcd61124e8d93aae1c` |

AMR 内 `0000000000000012.tar` 与 `build/dist.tar` 的缓冲区比较为相同。去掉 ZIP 中的应用 ID 目录前缀后，5 个文件均与 TAR 内容相同：

| 文件 | 字节数 | TAR 与 ZIP 共有的 SHA-256 |
| --- | ---: | --- |
| `appConfig.json` | 242 | `361ddc0714563a3dbb0f1d012d071d2e9e901107acf61c83e3f13d2b0ee7bd54` |
| `index.html` | 1604 | `71518be724ad21173e254b9c3992616b6a23592794c0c5ee83bc8517c7f9af74` |
| `index.js` | 29949 | `87c7f45e5f95b1760ea48969871161bae35c3929896ef2d5fee944988d3efb79` |
| `index.worker.js` | 10507 | `b27875027b9bc9ec377c5d3d86837ed06fc500fa8ade285b8ecf991855b97861` |
| `manifest.json` | 74 | `51542cb46c9528b6bf68680dc270299cb193c0086383d6b3956dbdfc0cbb57f2` |

## 目标环境验收

目标环境还需确认服务端产品版本、客户端小程序运行时能力与所用编译器的对应关系；使用独立测试版本上传 ZIP，并按 `data.success` 和 `data.data` 验证业务成功及资源 ID。随后查询资源信息、创建适用的测试发布任务，在目标 Android/iOS（如适用还包括 Harmony）客户端启动首个页面，验证页面渲染、桥接 API、跳转与资源加载。

只有这些结果通过，才能将本报告的“归档和内容核查通过”提升为“目标环境兼容性验收通过”。当前未执行上述网络、发布和客户端验证。


## 初次整合的验证结果

本次用新流程在临时目录重新编译最小项目，没有覆盖以上历史基线。新 ZIP 与 AMR 均通过结构检查，AMR 明确返回 `signatureVerified: false`；正式上传流程使用 ZIP，由后台处理资源包。

- Node 自动化测试 17 项通过，覆盖归档内容、入口、元数据传递、异常清理、路径和源文件保护。
- Rust 自动化测试 18 项通过，覆盖 Java 协议对应的公钥验签、UTF-8 multipart、业务响应、发布状态确认、失败后恢复 ID、超时和禁止重试/重定向。
- macOS arm64 和 Linux x64 单文件均实际完成“编译 → ZIP → RSA 上传 → 创建任务 → 查询待发布 → 启动 → 确认发布中”的本机模拟后台集成测试；另验证创建任务失败时保留 `packageId` 并停止后续操作。模拟使用临时生成的密钥，没有使用参考工程密钥或目标后台凭据。
- 发布编译在独立子进程中执行，编译器日志写入 stderr，最终发布结果可以严格作为一份 JSON 解析。测试还将首页改为 `pages/start/home`，确认没有沿用原样例的固定入口。
- 单文件在 Linux `debian:bookworm-slim` 的无网络环境和 macOS 隔离 PATH/禁止网络环境中完成实际编译及包校验，无需目标机器安装 Node、Rust、Java 或 minidev。
- Windows x64 已配置原生 CI、Rust 静态 CRT、运行库依赖检查和相同集成测试；本机未执行 Windows 运行验证。

上述模拟证明客户端和交付文件的整合行为。真实 mPaaS 服务的验签、资源处理、正式任务以及客户端安装/运行仍属于前述目标环境验收范围。

## 自动版本与生产发布的验证结果

只读 demo 已按用户确认的验证结果[归档](../archive/openapi-demo-2026-11-17/README.md)，原产物字节及校验值保持不变。用户确认的是查询 demo；本轮新增上传发布功能使用临时密钥与本机模拟后台验证。

- Rust 42 项测试通过，覆盖完整分页、数字版本比较、`0.0.19 → 0.1.0`、首次 `0.0.1`、无效列表/版本阻断，以及已有资源和任务的发布恢复。
- Node 17 项测试通过。Linux x64 与 macOS arm64 独立客户端均通过自动版本、只上传、上传发布、待发布任务恢复和已发布中只查询的模拟验证。
- 两个平台的完整单文件均实际执行“查询下一版本 → 编译 ZIP → 上传前复查 → 上传发布”，确认新首页和编译元数据保留；创建任务失败时保留资源 ID，编译后版本已变化时不上传。
- Linux 另在禁网的干净 Debian 环境完成编译和 ZIP/AMR 校验，无需外装 Node、Rust、Java 或 minidev。
- Windows 生产客户端已重新交叉编译并检查 AMD64 PE 与系统 DLL 导入，无额外运行库 DLL；本轮未在 Windows 实机执行新增上传发布流程。

生产包与操作方式见 [自动上传与发布](publishing.md)。后台任务确认达到 `taskStatus: 1` 表示“发布中”，不代表客户端已安装或页面已启动。

## 上一版上传信息的继承

上传按最新数字版本选取对应资源，查询资源详情补齐名称、客户端版本范围、平台、安装与运行开关、入口及扩展参数；名称或虚拟域名仍缺失时，调用 `getAppList` 按小程序 ID 匹配应用信息。资源的 `extraData` 仅提取已确认的运行开关，服务端 ID、下载地址和 `memo` 不作为上传默认值转发。

显式配置优先；可选字段的 `null` 可以清空旧值，显式 `0` 不会被误判为缺省。`extendInfo.launchParams` 按字段合并，本次编译的入口和启动参数覆盖历史对应值。ZIP 与图标仍是本次必须提供的本地文件。

本轮 Rust 55 项、Node 18 项测试通过。模拟后台验证覆盖最简上传配置继承、最新资源选择、应用域名补缺、显式覆盖、默认值只读预览、无效详情阻断，以及完整编译流程中的名称与启动参数继承。真实后台的新增上传发布流程仍需目标环境验收。
