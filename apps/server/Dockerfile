# Build context: apps/server. Run scripts/bundle-dashboard.sh first to ship
# the dashboard; without it the image serves the API only.
ARG FEATURES=sqlite

FROM lukemathwalker/cargo-chef:0.1.78-rust-1.98-slim-bookworm AS chef
RUN apt-get update && apt-get install -y --no-install-recommends build-essential \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ARG FEATURES
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --locked --no-default-features --features ${FEATURES} --recipe-path recipe.json
COPY . .
RUN cargo build --release --locked --no-default-features --features ${FEATURES}
RUN mkdir -p /data /app/static && chown -R 65532:65532 /data /app/static

FROM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app
COPY --from=builder /app/target/release/llmtrack /app/llmtrack
COPY --chown=65532:65532 --from=builder /app/static /app/static
COPY --chown=65532:65532 --from=builder /data /data
VOLUME /data
EXPOSE 4000
USER nonroot
CMD ["/app/llmtrack"]
