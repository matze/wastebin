# Changelog

## Unreleased

### Fixed

- Set bright color for textarea in dark mode.


## 1.2.0

**2022-06-08**

### Changed

- Timeout with status code 408 after five seconds.
- Add <kbd>r</kbd> and <kbd>n</kbd> keybinds on the paste view.
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
