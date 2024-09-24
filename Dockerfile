# --- build stage ---
FROM rust:1.80 AS builder

# Add targets for both aarch64 and x86_64
RUN rustup target add aarch64-unknown-linux-musl x86_64-unknown-linux-musl && \
    apt-get update && \
    apt-get install -y musl-tools musl-dev && \
    update-ca-certificates

ENV USER=app
ENV UID=10001

# Create a user
RUN adduser \
    --disabled-password \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"

WORKDIR /app
COPY . .

# Build both binaries for both architectures
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --target aarch64-unknown-linux-musl --release && \
    cargo build --target x86_64-unknown-linux-musl --release

# --- final stage ---
FROM scratch

# Set ARG for architecture detection
ARG TARGETARCH

# Copy common files
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

# Conditionally copy files based on architecture
WORKDIR /app

# Handle aarch64 architecture
COPY --from=builder /app/target/aarch64-unknown-linux-musl/release/wastebin /app/wastebin-arm64
COPY --from=builder /lib/aarch64-linux-gnu/libgcc_s.so.1 /lib/aarch64-linux-gnu/libgcc_s.so.1
COPY --from=builder /lib/aarch64-linux-gnu/libm.so.6 /lib/aarch64-linux-gnu/libm.so.6
COPY --from=builder /lib/aarch64-linux-gnu/libc.so.6 /lib/aarch64-linux-gnu/libc.so.6
COPY --from=builder /lib/ld-linux-aarch64.so.1 /lib/ld-linux-aarch64.so.1

# Handle x86_64 architecture
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/wastebin /app/wastebin-x86_64

USER app:app

# Automatically choose the correct binary based on architecture
CMD if [ "$TARGETARCH" = "arm64" ]; then \
        /app/wastebin-arm64; \
    else \
        /app/wastebin-x86_64; \
    fi
