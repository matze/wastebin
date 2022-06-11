# wastebin

[![Rust](https://github.com/matze/wastebin/actions/workflows/rust.yml/badge.svg)](https://github.com/matze/wastebin/actions/workflows/rust.yml)

A minimal pastebin shamelessly copied from
[bin](https://github.com/WantGuns/bin). Things different from bin:

* sqlite3 and axum backend
* light/dark mode
* paste expiration
* line numbers

<p align="center"><img src="https://raw.githubusercontent.com/matze/wastebin/master/assets/screenshot.webp"></p>

<p align="center"><strong><a href="https://wastebin-pkue.onrender.com">DEMO</a></strong> (might be a bit slow on first request 🐌)</p>


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


### Browser interface

On a paste view you can use <kbd>r</kbd> and <kbd>n</kbd> to go to the raw view
and back to the index page. Furthermore, you can use <kbd>y</kbd> to copy the
paste URL to the clipboard.


## Configuration

The following environment variables can be set to configure the server:

* `WASTEBIN_ADDRESS_PORT` string that determines which address and port to bind
  a. If not set, it binds by default to `0.0.0.0:8088`.
* `WASTEBIN_CACHE_SIZE` number of rendered syntax highlight items to cache.
  Defaults to 128 and can be disabled by setting to 0.
* `WASTEBIN_DATABASE_PATH` path to the sqlite3 database file. If not set, an
  in-memory database is used.
* `WASTEBIN_MAX_BODY_SIZE` number of bytes to accept for POST requests. Defaults
  to 1 MB.
* `WASTEBIN_TITLE` overrides the HTML page title. Defaults to `wastebin`.

Additionally you can use the `RUST_LOG` environment variable to influence
logging. Besides the typical `trace`, `debug`, `info` etc. keys, you can also
set the `tower_http` key to some log level to get additional information request
and response logs.


## API endpoints

Wastebin exposes endpoints to submit a paste and retrieve it again.

POST a new paste to the `/api/entries` endpoint with the following JSON
payload:

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

To retrieve the raw content, make a GET request on the `/api/entries/:id`
route, with `:id` being the identifier _without_ the file extension.


### Paste from clipboard

We can use the API POST endpoint to paste clipboard data easily from the command
line using `xclip`, `curl` and `jq`. Just define the following function in your
`.bashrc` and you are good to go:

```bash
function waste-paste() {
    local URL=$(\
        jq -n --arg t "$(xclip -selection clipboard -o)" '{text: $t}' | \
        curl -s -H 'Content-Type: application/json' --data-binary @- http://0.0.0.0:8088/api/entries | \
        jq -r '. | "http://0.0.0.0:8088\(.path)"')

    xdg-open $URL
}
```
