# The rust:*-alpine images are already musl-hosted (the host triple is
# <arch>-unknown-linux-musl), so cargo produces a statically linked binary for
# whichever architecture this image is built on. Naming an explicit --target
# would pin the build to one architecture and break multi-arch builds.
FROM rust:1.98-alpine AS builder

RUN apk add --no-cache musl-dev

WORKDIR /build
COPY . .
RUN cargo build --release

FROM scratch

# revcheck reads a JSON payload on stdin and makes no network calls, so the
# image needs no CA bundle, no shell and no writable filesystem.
COPY --from=builder /build/target/release/revcheck /revcheck
ENTRYPOINT ["/revcheck"]
