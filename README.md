# wastebin

A minimal pastebin shamelessly copied from
[bin](https://github.com/WantGuns/bin). Things different from bin:

* sqlite3 and axum backend
* light/dark mode
* paste expiration

## Configuration

The following environment variables can be set to configure the server:

* `WASTEBIN_DATABASE_PATH` path to the sqlite3 database file. If not set, an
  in-memory database is used.
* `WASTEBIN_ADDRESS_PORT` string that determines which address and port to bind
  to. If not set, it binds by default to `0.0.0.0:8088`.
