# <img width="24px" height="24px" style="position: relative; top: 2px;" src="assets/favicon.png"/> wastebin

[![Rust](https://github.com/matze/wastebin/actions/workflows/rust.yml/badge.svg)](https://github.com/matze/wastebin/actions/workflows/rust.yml)

A minimal pastebin with a design shamelessly copied from
[bin](https://github.com/WantGuns/bin).

<p align="center"><img src="https://raw.githubusercontent.com/matze/wastebin/master/assets/screenshot.webp"></p>

<p align="center"><strong><a href="https://bin.bloerg.net">DEMO</a></strong> (resets every day)</p>


## Features

* [axum](https://github.com/tokio-rs/axum) and [sqlite3](https://www.sqlite.org) backend
* comes as a single binary with low memory footprint
* provides paste compression using [zstd](https://github.com/facebook/zstd)
* highlights entries with [syntect](https://github.com/trishume/syntect)
* has seven color themes in light and dark mode
* encrypts entries using ChaCha20Poly1305 and argon2 hashed passwords
* allows deletion after expiration, after reading or by anonymous owners
* shows QR code to browse a paste's URL on mobile devices

### Non-features

* user authentication and admin functionality
* arbitrary file uploads
* mitigations for all kinds of DoS attack vectors


## Installation

### Build from source

Install a Rust 2021 toolchain containing Rust 1.70 with
[rustup](https://rustup.rs) and run the server binary with

    $ cargo run --release


### Run pre-built binaries

You can also download pre-built, statically compiled [Linux
binaries](https://github.com/matze/wastebin/releases). After extraction run the
contained `wastebin` binary.

### Build a container image

It's possible to build a container image using Docker or Podman. Assuming you're in the root directory of repository run
```bash
$ sudo docker build -t wastebin:v2.4.3 -f Dockerfile .
```
for Docker or
```bash
$ podman build -t wastebin:v2.4.3 -f Dockerfile
```
for Podman.

To cross-compile, make sure that your container engine of choice supports it,
e.g. Docker:

```bash
$ sudo docker buildx ls
NAME/NODE     DRIVER/ENDPOINT   STATUS    BUILDKIT   PLATFORMS
default*      docker
 \_ default    \_ default       running   v0.14.1    linux/amd64, linux/amd64/v2, linux/386, linux/arm64, linux/riscv64, linux/ppc64, linux/ppc64le, linux/s390x, linux/mips64le, linux/mips64, linux/loong64, linux/arm/v7, linux/arm/v6
```

To build an arm64 image on an x86_64 host run
```bash
$ sudo docker build --platform linux/arm64 -t wastebin:v2.4.3-arm64 -f Dockerfile.arm .
```
or
```bash
$ podman build --arch=arm64 -t wastebin:v2.4.3-arm64 -f Dockerfile.arm
```

### Run a Docker image

Alternatively, you can run a pre-built Docker image pushed to `quxfoo/wastebin`.
Here is how to persist the database as `state.db` via the
`WASTEBIN_DATABASE_PATH` environment variable and a bind mount to
`/path/for/storage`:

    $ docker run -e WASTEBIN_DATABASE_PATH=/data/state.db -v /path/for/storage:/data quxfoo/wastebin:latest

**NOTE**: The image is based on scratch which means it neither comes with a
shell nor with `TMPDIR` being set. If database migrations fail with an extended
sqlite error code 6410, pass `TMPDIR` pointing to a location, sqlite can write
to.


### Run with docker-compose
```
services:
  wastebin:
    restart: always
    environment:
      - WASTEBIN_DATABASE_PATH=/data/state.db
    ports:
      - "8088:8088"
    volumes:
      - './data:/data'
    image: 'quxfoo/wastebin:latest'
```
Make sure the `./data` folder is writable by the user 10001.


### Run with Nix

For Nix users, a `flake.nix` is also provided. Build and execute it directly
with:

```bash
nix run 'github:matze/wastebin#wastebin'
```

Or install the provided `wastebin` package like you normally would.


## Usage

### Browser interface

When viewing a paste, you can use

* <kbd>r</kbd> to view the raw paste,
* <kbd>n</kbd> to go the index page,
* <kbd>y</kbd> to copy the current URL to the clipboard,
* <kbd>q</kbd> to display the current URL as a QR code and
* <kbd>p</kbd> to view the formatted paste,
* <kbd>?</kbd> to view the list of keybindings.

To paste some text you can also use the <kbd>ctrl</kbd>+<kbd>s</kbd> key
combination.


### Configuration

The following environment variables can be set to configure the server and
run-time behavior:

| Variable                          | Description                                                   | Default               |
| --------------------------------- | ------------------------------------------------------------- | --------------------- |
| `WASTEBIN_ADDRESS_PORT`           | Address and port to bind the server to.                       | `0.0.0.0:8088`        |
| `WASTEBIN_BASE_URL`               | Base URL for the QR code display.                             | User agent's `Host` header field used as an approximation. |
| `WASTEBIN_CACHE_SIZE`             | Number of rendered items to cache. Disable with 0.            | `128`                 |
| `WASTEBIN_DATABASE_PATH`          | Path to the sqlite3 database file.                            | `:memory:`            |
| `WASTEBIN_HTTP_TIMEOUT`           | Maximum number of seconds a request is processed until wastebin responds with 408. | `5` |
| `WASTEBIN_MAX_BODY_SIZE`          | Number of bytes to accept for POST requests.                  | `1048576`, i.e. 1 MB  |
| `WASTEBIN_MAX_PASTE_EXPIRATION`   | Maximum allowed lifetime of a paste in seconds. Disable with 0. | `0`                 |
| `WASTEBIN_PASSWORD_SALT`          | Salt used to hash user passwords used for encrypting pastes.  | `somesalt`            |
| `WASTEBIN_SIGNING_KEY`            | Key to sign cookies. Must be at least 64 bytes long.          | Random key generated at startup, i.e. cookies will become invalid after restarts and paste creators will not be able to delete their pastes. |
| `WASTEBIN_THEME`                  | Theme colors, one of `ayu`, `base16ocean`, `coldark`, `gruvbox`, `monokai`, `onehalf`, `solarized`. | `ayu` |
| `WASTEBIN_TITLE`                  | HTML page title.                                              | `wastebin`            |
| `RUST_LOG`                        | Log level. Besides the typical `trace`, `debug`, `info` etc. keys, you can also set the `tower_http` key to a log level to get additional request and response logs. |  |


### API endpoints

POST a new paste to the `/` (up to 2.7.1) or `/api` (current HEAD) endpoint with
the following JSON payload:

```
{
  "text": "<paste content>",
  "extension": "<file extension, optional>",
  "title": "<paste title, optional>",
  "expires": <number of seconds from now, optional>,
  "burn_after_reading": <true/false, optional>,
  "password": <password for encryption optional>,
}
```

After successful insertion, you will receive a JSON response with the path to
the newly created paste for the browser:

```json
{"path":"/Ibv9Fa.rs"}
```

To retrieve the raw content, make a GET request on the `/raw/:id` route. If you
use a client that is able to handle cookies you can delete the paste once again
using the cookie in the `Set-Cookie` header set during redirect after creation.

In case the paste was encrypted, pass the password via the `wastebin-password`
header.


### Paste from neovim

Use the [wastebin.nvim](https://github.com/matze/wastebin.nvim) plugin and paste
the current buffer or selection with `:WastePaste`.


### Paste from clipboard

We can use the API POST endpoint to paste clipboard data easily from the command
line using `xclip`, `curl` and `jq`. Define the following function in your
`.bashrc` and you are good to go:

```bash
function paste_from_clipboard() {
    local URL=$(\
        jq -n --arg t "$(xclip -selection clipboard -o)" '{text: $t}' | \
            curl -s -H 'Content-Type: application/json' --data-binary @- https://wastebin.tld/api | \
            jq -r '. | "https://wastebin.tld\(.path)"')

    xdg-open $URL
}
```

### Paste from stdin

To paste from stdin use the following function in your `.bashrc`:

```bash
function paste_from_stdin() {
    jq -Rns '{text: inputs}' | \
        curl  -s -H 'Content-Type: application/json' --data-binary @- https://wastebin.tld/api | \
        jq -r '. | "wastebin.tld\(.path)"'
}
```

It can be handy for creating pastes from logs or the output of commands, e.g.
`cat file.log | paste_from_stdin`.


## License

[MIT](./LICENSE)
