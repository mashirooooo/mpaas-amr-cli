# mPaaS AMR CLI

验证 `minidev` 构建并封装 mPaaS `.amr` 发布包的独立工程。

## 安装

```bash
npm install
```

## 构建 AMR

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
```

首次运行单文件时，资源会释放到系统临时目录，后续构建不需要 Node、minidev 安装包或联网下载构建器。打包前必须在目标平台执行一次 `download-assets --with-compiler`。

## 独立可执行文件

在 Linux x64 或 Windows x64 目标环境上执行：

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

每个文件都是对应平台的单文件可执行程序。当前交付目标为 Linux 和 Windows；macOS 代码路径暂时保留，但不作为发布产物。独立文件需要能够执行 `build` 和 `validate`，并生成符合样例结构的 `.amr`：

```text
<appId>.amr (ZIP)
├── Manifest.xml
├── CERT.json
└── <appId>.tar
```

`package:sea` 使用当前机器的 Node SEA 能力打包。必须分别在 Linux x64 和 Windows x64 构建，或者使用项目内的 CI matrix：

```text
Linux   -> mpaas-amr-linux-x64 / mpaas-amr-linux-arm64
Windows -> mpaas-amr-win32-x64.exe / mpaas-amr-win32-arm64.exe
```

Linux 可在 Docker 中验证：

```bash
docker build --platform linux/amd64 -f docker/linux.Dockerfile -t mpaas-amr-linux .
```

Windows 需要在 Windows Docker 主机或 Windows CI runner 中执行同等流程；Linux Docker 主机不能运行 Windows 容器和 Windows 构建器。

项目内的 `.github/workflows/build.yml` 会分别在 `ubuntu-22.04` 和 `windows-2022` 上下载构建器、生成单文件并执行验证。当前 macOS 不属于交付目标。

## 上传发布

本工具负责构建和生成 `.amr`，不包含 mPaaS 控制台登录、上传和发布接口调用。生成文件后，在 mPaaS 控制台进入对应小程序的版本管理/离线包上传入口，上传：

```text
artifacts/<version>/<appId>.amr
```

当前 `CERT.json` 按样例生成空签名字段，仅用于验证包结构和流程。若目标 mPaaS 环境校验正式签名，需要接入企业已有的 mPaaS 签名能力或使用 IDE/官方上传流程生成签名，不能把空字段直接用于正式发布。
