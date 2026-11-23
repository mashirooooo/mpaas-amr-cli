FROM --platform=linux/amd64 rust:1-bookworm AS openapi-build
WORKDIR /build
COPY rust-sdk/Cargo.toml rust-sdk/Cargo.lock ./
COPY rust-sdk/src ./src
RUN cargo build --release --locked

FROM --platform=linux/amd64 node:22.14.0-bookworm AS cli-build
WORKDIR /workspace
ENV MINIDEV_SKIP_DOWNLOAD_ASSETS=1
ENV PATH="/workspace/node_modules/.bin:${PATH}"
COPY package.json package-lock.json ./
RUN npm ci
RUN minidev download-assets --with-compiler
COPY . .
COPY --from=openapi-build /build/target/release/mpaas-openapi-rs ./rust-sdk/target/release/mpaas-openapi-rs
ENV SEA_OPENAPI_BINARY=/workspace/rust-sdk/target/release/mpaas-openapi-rs
RUN npm test
RUN --network=none node scripts/verify-deploy.mjs ./rust-sdk/target/release/mpaas-openapi-rs --client
RUN npm run package:sea
RUN --network=none node scripts/verify-release.mjs ./dist/mpaas-amr-linux-x64

FROM --platform=linux/amd64 debian:bookworm-slim AS standalone
WORKDIR /app
COPY --from=cli-build /workspace/dist/mpaas-amr-linux-x64 ./mpaas-amr
COPY --from=openapi-build /build/target/release/mpaas-openapi-rs ./mpaas-openapi-rs
COPY fixtures/minimal /project
RUN --network=none ./mpaas-amr --help
RUN --network=none ./mpaas-amr openapi --client-help
RUN --network=none ./mpaas-openapi-rs --help
RUN --network=none ./mpaas-amr build --project /project --app-id 0000000000000012 --version 1.0.0.0 --format both --output artifacts
RUN --network=none ./mpaas-amr validate artifacts/1.0.0.0/0000000000000012.zip
RUN --network=none ./mpaas-amr validate artifacts/1.0.0.0/0000000000000012.amr
ENTRYPOINT ["./mpaas-amr"]
CMD ["--help"]
