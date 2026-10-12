// Its expectation cannot hold: tests/playwright_import_chrome.rs runs the
// generated test and checks that it fails, which shows the converted
// expectations really check the page.
import { test, expect } from '@playwright/test';

test('expects a heading the page does not have', async ({ page }) => {
  await page.goto('http://127.0.0.1:4173/');
  await expect(page.locator('h1')).toHaveText('Not the heading', { timeout: 2000 });
});
