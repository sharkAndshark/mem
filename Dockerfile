FROM rust:1-bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY tests ./tests

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libgcc-s1 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/mem /usr/local/bin/mem

STOPSIGNAL SIGINT
ENTRYPOINT ["mem"]
CMD ["-c", "50%", "-m", "50%"]
