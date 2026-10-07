# The mock provider, as a container next to the gateways it feeds.
FROM rust:1.98-slim-bookworm AS build
WORKDIR /bench
COPY . .
RUN cargo build --release --locked --bin llmtrack-bench

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /bench/target/release/llmtrack-bench /llmtrack-bench
EXPOSE 9000
ENTRYPOINT ["/llmtrack-bench", "mock", "--port", "9000"]
