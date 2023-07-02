# <img width="24px" height="24px" style="position: relative; top: 2px;" src="assets/favicon.png"/> wastebin

[![Rust](https://github.com/matze/wastebin/actions/workflows/rust.yml/badge.svg)](https://github.com/matze/wastebin/actions/workflows/rust.yml)

A minimal pastebin with a design shamelessly copied from
[bin](https://github.com/WantGuns/bin).

<p align="center"><img src="https://raw.githubusercontent.com/matze/wastebin/master/assets/screenshot.webp"></p>

<p align="center"><strong><a href="https://bin.bloerg.net">DEMO</a></strong> (resets every day)</p>


## Features

* axum and sqlite3 backend storing compressed paste data
* single binary with low memory footprint
* drag 'n' drop upload
* deletion after expiration, reading or by owners
* light/dark mode
* highlightable line numbers
* QR code to browse a paste's URL on mobile devices


## Installation

### Build from source

Install a Rust 2021 toolchain containing Rust 1.70 with
[rustup](https://rustup.rs) and run the server binary with

    $ cargo run --release


### Run pre-built binaries

You can also download pre-built, statically compiled [Linux
binaries](https://github.com/matze/wastebin/releases). After extraction run the
contained `wastebin` binary.


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


## Usage

### Browser interface

When viewing a paste, you can use

* <kbd>r</kbd> to view the raw paste,
* <kbd>n</kbd> to go the index page,
* <kbd>y</kbd> to copy the current URL to the clipboard,
* <kbd>q</kbd> to display the current URL as a QR code and
* <kbd>p</kbd> to view the formatted paste,
* <kbd>?</kbd> to view the list of keybindings.


### Configuration

The following environment variables can be set to configure the server and
run-time behavior:

* `WASTEBIN_ADDRESS_PORT` string that determines which address and port to bind
  a. If not set, it binds by default to `0.0.0.0:8088`.
* `WASTEBIN_BASE_URL` string that determines the base URL for the QR code
  display. If not set, the user agent's `Host` header field is used as an
  approximation.
* `WASTEBIN_CACHE_SIZE` number of rendered syntax highlight items to cache.
  Defaults to 128 and can be disabled by setting to 0.
* `WASTEBIN_DATABASE_PATH` path to the sqlite3 database file. If not set, an
  in-memory database is used.
* `WASTEBIN_MAX_BODY_SIZE` number of bytes to accept for POST requests. Defaults
  to 1 MB.
* `WASTEBIN_SIGNING_KEY` sets the key to sign cookies. If not set, a random key
  will be generated which means cookies will become invalid after restarts and
  paste creators will not be able to delete their pastes anymore.
* `WASTEBIN_TITLE` overrides the HTML page title. Defaults to `wastebin`.
* `RUST_LOG` influences logging. Besides the typical `trace`, `debug`, `info`
  etc. keys, you can also set the `tower_http` key to some log level to get
  additional information request and response logs.


### API endpoints

POST a new paste to the `/` endpoint with the following JSON payload:

```
{
  "text": "<paste content>",
  "extension": "<file extension, optional>",
  "expires": <number of seconds from now, optional>,
  "burn_after_reading": <true/false, optional>
}
```

After successful insertion, you will receive a JSON response with the path to
the newly created paste:

```
{"path":"/Ibv9Fa.rs"}
```

To retrieve the raw content, make a GET request on the `/:id` route and an
accept header value that does not include `text/html`. If you use a client that
is able to handle cookies you can delete the paste once again using the cookie
in the `Set-Cookie` header set during redirect after creation.


### Paste from neovim

Use the [wastebin.nvim](https://github.com/matze/wastebin.nvim) plugin and paste
the current buffer or selection with `:WastePaste`.


### Paste from clipboard

We can use the API POST endpoint to paste clipboard data easily from the command
line using `xclip`, `curl` and `jq`. Define the following function in your
`.bashrc` and you are good to go:

```bash
function waste-paste() {
    local URL=$(\
        jq -n --arg t "$(xclip -selection clipboard -o)" '{text: $t}' | \
        curl -s -H 'Content-Type: application/json' --data-binary @- http://0.0.0.0:8088 | \
        jq -r '. | "http://0.0.0.0:8088\(.path)"')

    xdg-open $URL
}
```


## License

[MIT](./LICENSE)
