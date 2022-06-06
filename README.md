# wastebin

[![Rust](https://github.com/matze/wastebin/actions/workflows/rust.yml/badge.svg)](https://github.com/matze/wastebin/actions/workflows/rust.yml)

A minimal pastebin shamelessly copied from
[bin](https://github.com/WantGuns/bin). Things different from bin:

* sqlite3 and axum backend
* light/dark mode
* paste expiration
* line numbers

<p align="center"><img src="https://raw.githubusercontent.com/matze/wastebin/master/assets/screenshot.webp"></p>

<p align="center"><strong><a href="https://wastebin-pkue.onrender.com">DEMO</a></strong> (might be a bit slow 🐌)</p>


## Build from source

Install a Rust 2021 toolchain with [rustup](https://rustup.rs) and run the
server binary with

    $ cargo run --release


## Run pre-built binaries

We provide pre-built, statically compiled [Linux
binaries](https://github.com/matze/wastebin/releases). Just download and archive
and run the contained `wastebin` binary.

Alternative you can run a pre-built Docker image pushed to `quxfoo/wastebin`
with

    $ docker run quxfoo/wastebin:latest


## Configuration

The following environment variables can be set to configure the server:

* `WASTEBIN_DATABASE_PATH` path to the sqlite3 database file. If not set, an
  in-memory database is used.
* `WASTEBIN_ADDRESS_PORT` string that determines which address and port to bind
  a. If not set, it binds by default to `0.0.0.0:8088`.
* `WASTEBIN_CACHE_SIZE` number of rendered syntax highlight items to cache.
  Defaults to 128 and can be disabled by setting to 0.

Additionally you can use the `RUST_LOG` environment variable to influence
logging. Besides the typical `trace`, `debug`, `info` etc. keys, you can also
set the `tower_http` key to some log level to get additional information request
and response logs.
