# Changelog

## Unreleased

### Fixed

- Evict cached items for expired pastes.

### Changed

- Do not swallow fatal errors from serving and database purging.

### Added

- Link to error page to go back to the index.


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
