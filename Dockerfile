# syntax=docker/dockerfile:1
# Dockerfile oficial (o que a banca roda): solução Rust.
# Build estático (musl) e imagem final vazia: só o binário e as regras.
# Outras soluções: docker build -f solucao-go_cel/Dockerfile -t motor-go .
FROM rust:1.90-alpine AS base
WORKDIR /app/solucao-rust
COPY solucao-rust/Cargo.toml solucao-rust/Cargo.lock ./
COPY solucao-rust/src src
COPY solucao-rust/regras regras

# docker build --target teste .  -> roda os testes (unitários + bordas compartilhadas)
FROM base AS teste
COPY solucao-rust/tests tests
COPY testes /app/testes
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/solucao-rust/target \
    RUSTFLAGS="-C target-cpu=native" cargo test --release

FROM base AS build
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/solucao-rust/target \
    RUSTFLAGS="-C target-cpu=native" cargo build --release && cp target/release/motor /motor

FROM scratch
COPY --from=build /motor /motor
COPY solucao-rust/regras /regras
WORKDIR /
ENTRYPOINT ["/motor"]
