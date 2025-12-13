# Cross-compile Dockerfile supporting both x86_64-unknown-linux-musl and
# aarch64-unknown-linux-musl targets using zig to link against musl libc. Note
# that this is to be used from an x86_64 host.

# --- build image

FROM rust:1.90 AS builder

RUN rustup target add \
    aarch64-unknown-linux-musl \
    x86_64-unknown-linux-musl

RUN update-ca-certificates

ENV ZIGVERSION=0.15.2

RUN wget https://ziglang.org/download/$ZIGVERSION/zig-x86_64-linux-$ZIGVERSION.tar.xz && \
    tar -C /usr/local --strip-components=1 -xf zig-x86_64-linux-$ZIGVERSION.tar.xz && \
    mv /usr/local/zig /usr/local/bin && \
    rm zig-x86_64-linux-$ZIGVERSION.tar.xz

RUN cargo install --locked cargo-zigbuild

WORKDIR /app

COPY . .

RUN cargo zigbuild \
    --release \
    --target aarch64-unknown-linux-musl \
    --target x86_64-unknown-linux-musl \
    --bin wastebin \
    --bin wastebin-ctl

RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "10001" \
    "app"


# --- x86_64-unknown-linux-musl final image

FROM scratch AS amd64

COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

WORKDIR /app
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/wastebin ./
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/wastebin-ctl ./
USER app:app
CMD ["/app/wastebin"]

# --- aarch64-unknown-linux-musl final image

FROM scratch AS arm64

COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

WORKDIR /app
COPY --from=builder /app/target/aarch64-unknown-linux-musl/release/wastebin ./
COPY --from=builder /app/target/aarch64-unknown-linux-musl/release/wastebin-ctl ./
USER app:app
CMD ["/app/wastebin"]
