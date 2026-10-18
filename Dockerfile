# Build stage: a plain cargo build on Debian, so the binary links against
# the same glibc as the runtime image below.
FROM rust:1-slim-bookworm AS builder
RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked
# An empty, nonroot-owned directory for the runtime image's /data volume
# (distroless has no shell to mkdir with; uid 65532 is its nonroot user).
RUN mkdir /data && chown 65532:65532 /data

# Runtime stage: libc and libgcc only, which is all nm needs. No shell, no
# package manager.
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=builder /src/target/release/nm /usr/local/bin/nm
COPY --from=builder --chown=65532:65532 /data /data
# The session file goes under /data. Without a mount it lives in the
# container and is gone with --rm; to keep it, run as yourself and mount
# your own directory:
#   docker run --rm -it --user "$(id -u):$(id -g)" -v ~/.local/share/nm:/data/nm ghcr.io/nclark/nm
ENV XDG_DATA_HOME=/data
ENTRYPOINT ["nm"]
