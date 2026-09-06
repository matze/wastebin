import { test, expect } from '@playwright/test';

test("wastebin-qa-sentinal-record", async ({ page }) => {
  await page.goto("/");
  await expect(page).toHaveURL(new RegExp("/"));
  // an app may show a consent/welcome overlay on load that intercepts the first
  // interaction. Wait (BOUNDED — a streaming SPA never reaches networkidle, so an
  // unbounded wait hangs to the test timeout) for the app to settle so a late-rendered
  // overlay is present, then dismiss it (Escape) as a user would. Both guarded → no-op
  // on timeout / when nothing is open.
  await page.waitForLoadState('networkidle', { timeout: 3000 }).catch(() => {});
  await page.keyboard.press('Escape').catch(() => {});
  if (await page.locator("#filter").count().catch(() => 0)) await page.locator("#filter").fill("test", { timeout: 5000 }).catch(() => {});
  if (await page.locator("#langs").count().catch(() => 0)) await page.locator("#langs").selectOption("as", { timeout: 5000 }).catch(() => {});
  if (await page.locator("#text").count().catch(() => 0)) await page.locator("#text").fill("test", { timeout: 5000 }).catch(() => {});
  if (await page.locator("#title").count().catch(() => 0)) await page.locator("#title").fill("QA Sentinal Record", { timeout: 5000 }).catch(() => {});
  {
    const _cb = page.locator("input[name=\"expires\"][value=\"2592000\"]");
    if ((await _cb.count().catch(() => 0)) && (await _cb.isChecked().catch(() => null)) !== true) {
      const _lbl = _cb.locator('xpath=ancestor::label[1]');
      if (await _lbl.count().catch(() => 0)) await _lbl.first().click({ timeout: 5000 }).catch(() => {});
      else await _cb.check({ force: true, timeout: 5000 }).catch(() => {});
    }
  }
  await page.getByRole("button", { name: "paste", exact: true }).click();
  await expect(page).toHaveURL(new RegExp("/[^/]+"));
  await expect(page.locator("code").first()).not.toBeEmpty();
});
