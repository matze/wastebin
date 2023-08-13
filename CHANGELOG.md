# Changelog

## Unreleased

### Fixed

- Initial reading of pastes set to burn-after-reading.


## 2.4.0

**2023-08-11**

### Added

- `WASTEBIN_HTTP_TIMEOUT` environment variable to control request timeouts.

### Changed

- ⚠️ Database schema updated to version 6. Like previous migrations rolling back
  is not (easily) possible, so plan on making a backup in case you want to roll
  back the server itself.
- Allow optional encryption of pastes based on Argon 2 password hashing and
  ChaCha20/Poly1305 symmetric encryption.

### Fixed

- Language selection filter which was not working correctly with certain
  characters.


## 2.3.5

**2023-07-17**

### Added

- Additional syntaxes compiled by the [zola](https://github.com/getzola/zola)
  project.


## 2.3.4

**2023-06-29**

### Fixed

- Add anchors to line number, so the links actually make some sense.
- Do not highlight lines longer than 2048 characters. This can take a
  considerable amount of time effectively DoS'ing the server.


## 2.3.3

**2023-04-21**

### Added

- QR code display accessible via <kbd>q</kbd> to browse the URL on phones and
  corresponding `WASTEBIN_BASE_URL` environment variable to control the base. In
  case it is not set, the user agent's `Host` header field is used as an
  approximation. To go back to normal paste view you can use <kbd>p</kbd>.
- Help overlay accessible via <kbd>?</kbd>.

### Changed

- Serve style CSS filename based on content hash to force client reload on
  change. With that bump max age for CSS to six months.


## 2.3.2

**2023-03-04**

### Changed

- Replace overlaying link box with a navigation bar containing stylized buttons
  and homogenize layout in general.

### Fixed

- Format burn page like the rest.


## 2.3.1

**2023-02-04**

### Fixed

- Return correct exit code in case of errors.


## 2.3.0

**2023-02-01**

### Changed

- **Breaking**: replace deletion timer with a cookie based solution that
  identifies creator of a paste on subsequent visits. That cookie is a
  monotonically increasing number and only used to implement the delete
  functionality. Because that implies it is a strictly necessary cookie
  according to GDPR, we **will not show a cookie banner**. If you are
  uncomfortable with that either strip the `Set-Cookie` header from responses
  via a proxy server or stop using this software.
- **Breaking**: stop supporting down migrations.
- Compress data with zstd for a reduction of about 75%. On migration all rows
  will be compressed. However file size will not change but unused pages be used
  for new rows. If you want to reduce the file size, you have to use the
  `VACUUM` statement.
- The database is not purged periodically anymore, instead expired entries are
  removed on access.


## 2.2.1

**2023-01-10**

### Changed

- Upgraded to tokio 1.24.1 to mitigate RUSTSEC-2023-0001.


## 2.2.0

**2022-12-26**

### Changed

- Move to axum 0.6.

### Fixed

- <kbd>d</kbd> downloads again.


## 2.1.0

**2022-11-07**

### Added

- Paste text content by dragging and dropping files onto the text area.


## 2.0.1

**2022-10-14**

### Fixed

- Broken insertion via JSON API.


## 2.0.0

**2022-07-31**

### Changed

- **Breaking**: remove possibility to GET `/api/entries/:id`, just use `/:id`.
- **Breaking**: remove possibility to POST to `/api/entries` and DELETE
  `/api/entries/:id`, this can be done on `/` and `/:id` respectively. Note that
  DELETEing `/:id` will now return a 303 status code instead of 200.
- Return appropriate content type for `/:id` based on `accept` header (i.e.
  `text/html` returns the HTML page) and the `fmt` query parameter (i.e. set to
  `raw` returns raw text).
- Use `dl` query parameter to determine the extension to download a paste.
- Use `fmt=raw` query parameter to fetch plain text paste.
- Set cache control timeout for the favicon.


## 1.6.0

**2022-07-19**

### Changed

- Normal font color for the light theme to increase contrast.
- Strange content padding.


## 1.5.0

**2022-07-04**

### Added

- Link that is valid for one minute to delete a paste.
- `generator` meta tag containing the version number.


## 1.4.0

**2022-06-27**

### Fixed

- Evict cached items for expired pastes.

### Changed

- Do not swallow fatal errors from serving and database purging.

### Added

- Link to error page to go back to the index.
- Link to download a paste (@yannickfunk).
- Bind <kbd>d</kbd> to download a paste.


## 1.3.0

**2022-06-12**

### Added

- <kbd>y</kbd> keybind to copy the paste URL to the clipboard
- `WASTEBIN_TITLE` environment variable to override the HTML page title.

### Changed

- Reduced font size of pre and text area to 13pt.


## 1.2.1

**2022-06-11**

### Fixed

- Set bright color for textarea in dark mode.


## 1.2.0

**2022-06-08**

### Added

- Add <kbd>r</kbd> and <kbd>n</kbd> keybinds on the paste view.

### Changed

- Timeout with status code 408 after five seconds.
- Limit maximum body size to 1 MB or a value set with `WASTEBIN_MAX_BODY_SIZE`
  in bytes.


## 1.1.0

**2022-06-06**

### Added

- Configurable cache for syntax highlighted HTML fragments to improve response
  times, especially when run in debug mode.
- `/api/health` endpoint for render.com health checks.


## 1.0.0

**2022-06-02**

- Initial release.
