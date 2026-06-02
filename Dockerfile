# Velo web server — builds the frontend and the Rust server, then runs the
# single self-contained binary that serves both. Works on Railway, Render,
# Fly.io, or any VPS / Docker host.
#
# Build:  docker build -t velo .
# Run:    docker run -p 8080:8080 -v velo-data:/data \
#           -e VELO_ADMIN_EMAIL=you@example.com -e VELO_ADMIN_PASSWORD=secret \
#           -e VELO_PUBLIC_URL=https://mail.example.com velo

# ---------- Stage 1: build the web frontend ----------
FROM node:20-slim AS web
WORKDIR /app
COPY package.json package-lock.json* ./
RUN npm install
COPY . .
RUN npm run build:web

# ---------- Stage 2: build the Rust server ----------
FROM rust:1-slim AS server
WORKDIR /build
# System libs needed by native-tls (OpenSSL) at build time.
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
# Copy the whole src-tauri workspace. The desktop `velo` package is in the
# workspace but is NOT compiled — `-p velo-server` builds only the server and
# velo-core, so the Tauri/desktop dependencies are never pulled in.
COPY src-tauri ./src-tauri
WORKDIR /build/src-tauri
RUN cargo build -p velo-server --release

# ---------- Stage 3: runtime ----------
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
# The built frontend and the server binary.
COPY --from=web /app/dist ./dist
COPY --from=server /build/src-tauri/target/release/velo-server ./velo-server

# Persisted data lives at /data. Configure persistent storage through your
# host's dashboard (Railway "Volumes" mounted at /data, Render disk, etc.).
# NOTE: no `VOLUME` directive — Railway rejects Dockerfiles that use it; the
# mount is provided by the platform's volume feature instead.
# VELO_BIND is intentionally unset so hosts that inject $PORT (Railway/Render)
# are honoured; the server falls back to 0.0.0.0:8080 when neither is set.
ENV VELO_STATIC_DIR=/app/dist \
    VELO_CONTROL_DB=/data/control.db \
    VELO_DATA_DIR=/data
EXPOSE 8080

CMD ["/app/velo-server"]
