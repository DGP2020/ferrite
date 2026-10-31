# Isolated Linux verification container for Ferrite (ADR 0003, ADR 0006, ADR 0011)
FROM rust:1.82-slim-bookworm

RUN apt-get update && apt-get install -y --no-install-recommends \
    clang \
    llvm \
    gcc \
    libc6-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN rustup component add clippy rustfmt

WORKDIR /workspace

CMD ["cargo", "test"]
