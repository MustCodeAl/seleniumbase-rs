// A @playwright/test file against the loopback shop page that
// tests/playwright_import_chrome.rs serves. The importer tests replace
// http://127.0.0.1:4173 with the address the page is served on.
import { test, expect, type Page } from '@playwright/test';

const BASE = 'http://127.0.0.1:4173/';

test.describe('shop', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(BASE);
    await page.waitForLoadState('domcontentloaded');
  });

  test('has a title and a heading', async ({ page }) => {
    await expect(page).toHaveTitle('Shop Fixture');
    await expect(page).toHaveTitle(/Shop/);
    await expect(page).toHaveURL(BASE);
    await expect(page.getByRole('heading', { name: 'Welcome to the shop' })).toBeVisible();
    await expect(page.locator('h1')).toHaveText('Welcome to the shop');
    await expect(page.locator('#intro')).toContainText('fruit');
    const title: string = await page.title();
    expect(title).toBe('Shop Fixture');
    const url = page.url();
    expect(url).toContain('127.0.0.1');
  });

  test('fills in the order form', async ({ page }) => {
    await page.getByLabel('Email').fill('buyer@example.test');
    await expect(page.getByLabel('Email')).toHaveValue('buyer@example.test');
    await page.getByPlaceholder('you@example').clear();
    await page.getByPlaceholder('you@example').pressSequentially('second@example.test');
    const email = await page.locator('#email').inputValue();
    expect(email).toBe('second@example.test');
    await page.locator('#qty').fill('3');
    await page.locator('#fruit').selectOption('banana');
    await page.locator('#fruit').selectOption({ label: 'Cherry' });
    await page.locator('#fruit').selectOption({ index: 1 });
    await page.getByRole('checkbox').check();
    await expect(page.locator('#gift')).toBeChecked();
    await page.locator('#gift').uncheck();
    await expect(page.locator('#gift')).not.toBeChecked();
    await page.locator('#gift').setChecked(true);
    await expect(page.getByTestId('place-order')).toBeEnabled();
    await expect(page.locator('#disabled')).toBeDisabled();
    await page.getByRole('button', { name: 'Place order' }).click();
    await expect(page.locator('#status')).toHaveText('Ordered 3 banana as a gift');
    await expect(page).toHaveTitle('Ordered');
  });

  test('reads lists, links and attributes', async ({ page }) => {
    await expect(page.locator('#items li')).toHaveCount(3);
    const count = await page.locator('#items li').count();
    expect(count).toBe(3);
    await expect(page.locator('li').first()).toHaveText('Apple');
    await expect(page.locator('li').last()).toHaveText('Cherry');
    await expect(page.locator('li').nth(1)).toHaveText('Banana');
    await expect(page.locator('li').filter({ hasText: 'cherry' })).toBeVisible();
    await expect(page.getByText('Fresh fruit')).toBeVisible();
    await expect(page.getByText('Fresh fruit every day', { exact: true })).toBeVisible();
    await expect(page.getByAltText('Shop logo')).toBeVisible();
    await expect(page.getByTitle('About the shop')).toHaveAttribute('href', '/about');
    const href = await page.locator('a').getAttribute('href');
    expect(href).toBe('/about');
    const intro = await page.locator('#intro').textContent();
    expect(intro).toContain('fruit');
    const heading = await page.locator('h1').innerText();
    expect(heading).toBe('Welcome to the shop');
    const visible = await page.locator('#intro').isVisible();
    expect(visible).toBe(true);
    const gift = await page.locator('#gift').isChecked();
    expect(gift).toBeFalsy();
    await expect(page.locator('form >> #submit')).toBeVisible();
    await expect(page.locator('xpath=//h1')).toBeVisible();
    await expect(page.locator('text=Fresh fruit')).toBeVisible();
    await expect(page.locator('id=intro')).toBeVisible();
    await expect(page.locator('data-testid=place-order')).toBeVisible();
    await expect(page.locator('button:has-text("Place")')).toBeVisible();
    await expect(page.locator('#nothing-here')).toBeHidden();
  });

  test('navigates, waits and uses the keyboard and mouse', async ({ page }) => {
    await page.getByRole('link', { name: 'About us' }).click();
    await page.waitForURL('**/about');
    await expect(page).toHaveURL(/about$/);
    await page.goBack();
    await page.waitForSelector('#email');
    await page.goForward();
    await expect(page.locator('h1')).toHaveText('About us');
    await page.reload();
    await page.goto(BASE);
    await page.setViewportSize({ width: 900, height: 700 });
    await page.locator('#email').focus();
    await page.keyboard.type('kb@example.test');
    await page.keyboard.press('Tab');
    await page.locator('#email').press('End');
    await page.locator('#intro').hover();
    await page.locator('#intro').dblclick();
    await page.locator('#intro').click({ button: 'right' });
    await page.mouse.click(5, 5);
    await page.waitForTimeout(100);
    const sum = await page.evaluate(() => 20 + 22);
    expect(sum).toBe(42);
    await page.screenshot({ path: 'shop.png' });
    await page.locator('#email').blur();
    await page.locator('#upload').setInputFiles('Cargo.toml');
    const files = await page.evaluate('document.querySelector("#upload").files.length');
    expect(files).toBe(1);
  });

  test.skip('is skipped', async ({ page }) => {
    await page.goto(BASE);
  });

  test('keeps what it cannot convert as a TODO', async ({ page }) => {
    await page.route('**/api/**', route => route.abort());
    await expect(page.locator('h1')).toBeVisible();
  });
});
