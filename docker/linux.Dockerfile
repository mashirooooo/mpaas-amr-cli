FROM --platform=linux/amd64 node:22-bookworm

WORKDIR /workspace
COPY package.json package-lock.json ./

ENV MINIDEV_SKIP_DOWNLOAD_ASSETS=1
RUN npm ci

COPY . .
RUN node src/cli.mjs download-assets --with-compiler
RUN npm run build && npm run validate && npm run package:sea
RUN test -x dist/mpaas-amr-linux-x64
RUN ./dist/mpaas-amr-linux-x64 --help
RUN ./dist/mpaas-amr-linux-x64 validate artifacts/1.0.0.0/0000000000000012.amr
