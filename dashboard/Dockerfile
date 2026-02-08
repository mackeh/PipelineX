# syntax=docker/dockerfile:1.7

FROM rust:1.93-slim AS rust-builder
WORKDIR /build

RUN apt-get update && \
    apt-get install -y --no-install-recommends pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release --bin pipelinex

FROM node:20-bookworm-slim AS dashboard-builder
WORKDIR /app/dashboard

COPY dashboard/package.json dashboard/package-lock.json ./
RUN npm ci

COPY dashboard ./
RUN npm run build && npm prune --omit=dev

FROM node:20-bookworm-slim AS runtime
WORKDIR /app/dashboard

ENV NODE_ENV=production
ENV PORT=3000
ENV PIPELINEX_REPO_ROOT=/workspace
ENV PIPELINEX_PUBLIC_API_RATE_LIMIT_FILE=/data/public-api-rate-limits.json
ENV PIPELINEX_PUBLIC_API_AUDIT_LOG_FILE=/data/public-api-audit.log

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates git && \
    rm -rf /var/lib/apt/lists/*

COPY --from=dashboard-builder /app/dashboard/package.json ./package.json
COPY --from=dashboard-builder /app/dashboard/package-lock.json ./package-lock.json
COPY --from=dashboard-builder /app/dashboard/node_modules ./node_modules
COPY --from=dashboard-builder /app/dashboard/.next ./.next
COPY --from=dashboard-builder /app/dashboard/public ./public
COPY --from=dashboard-builder /app/dashboard/next.config.ts ./next.config.ts

RUN mkdir -p /app/target/release /workspace /data && chown -R node:node /app /workspace /data
COPY --from=rust-builder /build/target/release/pipelinex /app/target/release/pipelinex
RUN chmod +x /app/target/release/pipelinex

USER node
EXPOSE 3000

CMD ["npm", "run", "start", "--", "--hostname", "0.0.0.0", "--port", "3000"]
