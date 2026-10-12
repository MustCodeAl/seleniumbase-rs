// A plain Puppeteer script against the loopback shop page that
// tests/puppeteer_import_chrome.rs serves.
const puppeteer = require('puppeteer');
const assert = require('assert');

(async () => {
  const browser = await puppeteer.launch({ headless: 'new' });
  const page = await browser.newPage();
  await page.setViewport({ width: 1024, height: 768 });
  await page.setUserAgent('ShopBot/1.0');
  await page.goto('http://127.0.0.1:4173/', { waitUntil: 'networkidle0' });
  await page.waitForSelector('#email', { visible: true });
  await page.type('#email', 'pptr@example.test');
  await page.focus('#qty');
  await page.keyboard.press('ArrowUp');
  await page.select('#fruit', 'cherry');
  await page.click('#gift');
  await page.hover('#intro');
  const heading = await page.$eval('h1', el => el.textContent);
  const items = await page.$$eval('#items li', els => els.length);
  const email = await page.$eval('#email', el => el.value);
  const agent = await page.evaluate(() => navigator.userAgent);
  console.log(heading, items, email, agent);
  assert.strictEqual(heading, 'Welcome to the shop');
  assert.equal(items, 3);
  assert.strictEqual(email, 'pptr@example.test');
  assert.strictEqual(agent, 'ShopBot/1.0');
  await Promise.all([
    page.waitForNavigation(),
    page.click('a[href="/about"]'),
  ]);
  await page.waitForFunction('location.pathname === "/about" && document.title === "About"');
  const title = await page.title();
  if (title !== 'About') {
    throw new Error('wrong page: ' + title);
  }
  await page.goBack();
  await page.waitForSelector('xpath///h1');
  // A new navigation, so the form starts empty (going back may restore it).
  await page.goto('http://127.0.0.1:4173/');
  await page.select('#fruit', 'cherry');
  await page.click('#gift');
  await page.click('text/Place order');
  await page.waitForSelector('::-p-text(cherry as a gift)');
  const status = await page.$eval('#status', el => el.textContent);
  assert.ok(status.includes('cherry'));
  const content = await page.content();
  console.log(content.length);
  await new Promise(resolve => setTimeout(resolve, 100));
  await page.screenshot({ path: 'puppeteer.png' });
  await browser.close();
})();
