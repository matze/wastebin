# --- build image

FROM rust:1.90 AS builder

RUN rustup target add x86_64-unknown-linux-musl
RUN apt update && apt install -y musl-tools musl-dev
RUN update-ca-certificates

ENV USER=app
ENV UID=10001

RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"

WORKDIR /app
COPY . .
RUN cargo build --target x86_64-unknown-linux-musl --release


# --- final image

FROM scratch

COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

WORKDIR /app
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/wastebin ./
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/wastebin-ctl ./
USER app:app
CMD ["/app/wastebin"]
