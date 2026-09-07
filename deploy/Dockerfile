# 多阶段构建：前端 → Rust → 运行镜像
# 用法：docker build -f deploy/Dockerfile -t zerror-server .

# ---------- 1. 构建管理后台 ----------
FROM node:22-bookworm-slim AS web
WORKDIR /src/web
COPY web/package.json web/package-lock.json* ./
RUN npm install --no-audit --no-fund
COPY web/ ./
RUN NODE_OPTIONS=--max-old-space-size=2048 npm run build

# ---------- 2. 构建服务端（前端产物通过 rust-embed 嵌入） ----------
FROM rust:1-bookworm AS server
WORKDIR /src
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY server/Cargo.toml server/Cargo.lock* ./server/
# 预编译依赖层
RUN mkdir -p server/src web/dist && echo 'fn main() {}' > server/src/main.rs && echo '<!doctype html>' > web/dist/index.html \
    && cd server && cargo build --release && rm -rf src target/release/zerror-server target/release/deps/zerror_server-*
COPY server/ ./server/
COPY --from=web /src/web/dist ./web/dist
RUN cd server && cargo build --release && strip target/release/zerror-server

# ---------- 3. 运行 ----------
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 10001 -d /data zerror && mkdir -p /data && chown zerror:zerror /data
COPY --from=server /src/server/target/release/zerror-server /usr/local/bin/zerror-server
USER zerror
ENV ZERROR_DATA_DIR=/data \
    ZERROR_BIND=0.0.0.0:3000 \
    ZERROR_LOG=info
VOLUME ["/data"]
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s CMD curl -fsS http://127.0.0.1:3000/api/status || exit 1
ENTRYPOINT ["/usr/local/bin/zerror-server"]
