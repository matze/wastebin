# wastebin end-to-end tests

Functional browser tests for wastebin's core flows, written with [Playwright](https://playwright.dev).
They are **self-contained** and **independent of the project's build/CI** — they drive a running
wastebin instance over HTTP, so they add coverage without changing anything about how wastebin
is built or tested today.

## Run locally

```bash
# 1. Start wastebin, e.g.:
docker run -p 8088:8088 quxfoo/wastebin

# 2. Run the tests (from this directory):
npm install
npm run test:install           # one-time: download the Chromium browser
npx playwright test            # uses BASE_URL, default http://localhost:8088
```

Point the suite at any instance with `BASE_URL`:

```bash
BASE_URL=http://localhost:8088 npx playwright test
```

## What's covered
- **Create & view a paste** — open the home page, fill the paste form and submit, then follow the redirect to the paste view and assert the pasted content is rendered back.
