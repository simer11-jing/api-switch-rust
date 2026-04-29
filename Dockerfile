FROM rustlang/rust:nightly-alpine AS builder

RUN apk add --no-cache musl-dev sqlite-dev openssl-dev openssl-libs-static pkgconf

WORKDIR /app
COPY Cargo.toml ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release 2>/dev/null || true

COPY src ./src
COPY static ./static
RUN touch src/main.rs && cargo build --release

FROM alpine:3.19

RUN apk add --no-cache ca-certificates sqlite-libs
COPY --from=builder /app/target/release/api-switch /app/api-switch
COPY static /app/static

WORKDIR /app
EXPOSE 9091

CMD ["./api-switch"]
