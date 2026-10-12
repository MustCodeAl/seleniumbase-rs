const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({
    headless: false
  });
  const context = await browser.newContext();
  const page = await context.newPage();
  await page.goto('http://127.0.0.1:4173/');
  await page.getByLabel('Email').click();
  await page.getByLabel('Email').fill('codegen@example.test');
  await page.getByLabel('Email').press('Tab');
  await page.getByRole('spinbutton', { name: 'Quantity' }).fill('2');
  await page.getByLabel('Fruit').selectOption('cherry');
  await page.getByLabel('Gift wrap').check();
  await page.getByRole('button', { name: 'Place order' }).click();
  await page.getByText('Ordered 2 cherry as a gift').click();
  await page.getByRole('link', { name: 'About us' }).click();

  // ---------------------
  await context.close();
  await browser.close();
})();
