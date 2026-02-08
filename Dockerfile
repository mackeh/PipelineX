# Multi-stage build for minimal final image
FROM rust:1.93-slim AS builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev git && \
    rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build release binary
RUN cargo build --release --bin pipelinex

# Final minimal image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates git && \
    rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /build/target/release/pipelinex /usr/local/bin/pipelinex

# Create workspace directory
WORKDIR /workspace

# Set git safe directory for mounted volumes
RUN git config --global --add safe.directory /workspace

# Default command shows help
ENTRYPOINT ["pipelinex"]
CMD ["--help"]

# Labels
LABEL org.opencontainers.image.title="PipelineX"
LABEL org.opencontainers.image.description="CI/CD Pipeline Bottleneck Analyzer & Auto-Optimizer"
LABEL org.opencontainers.image.url="https://github.com/mackeh/PipelineX"
LABEL org.opencontainers.image.source="https://github.com/mackeh/PipelineX"
LABEL org.opencontainers.image.vendor="mackeh"
LABEL org.opencontainers.image.licenses="MIT"
