// A jest-puppeteer test (the global `page`) against the loopback shop page.
import 'expect-puppeteer';

const BASE: string = 'http://127.0.0.1:4173/';

describe('shop (jest-puppeteer)', () => {
  beforeAll(async () => {
    await page.goto(BASE);
  });

  beforeEach(async () => {
    await page.goto(BASE);
  });

  it('shows the heading', async () => {
    await expect(page).toMatchElement('h1', { text: 'Welcome' });
    await expect(page).toMatchTextContent('Fresh fruit');
    const heading = await page.$eval('h1', (el) => el.textContent);
    expect(heading).toBe('Welcome to the shop');
    expect(heading).toContain('shop');
    expect(heading).toMatch(/Welcome/);
    const title = await page.title();
    expect(title).toEqual('Shop Fixture');
    const items = await page.$$('#items li');
    expect(items.length).toBe(3);
    const href = await page.$eval('a', (el) => el.getAttribute('href'));
    expect(href).toBe('/about');
  });

  it('orders with the locator API and element handles', async () => {
    await page.locator('#email').fill('jest@example.test');
    await page.locator('#intro').hover();
    await page.locator('#submit').wait();
    const gift = await page.$('#gift');
    await gift.click();
    await page.select('#fruit', 'banana');
    const upload = await page.$('#upload');
    await upload.uploadFile('Cargo.toml');
    await expect(page).toFill('#qty', '7');
    await expect(page).toClick('button', { text: 'Place order' });
    await page.waitForSelector('::-p-text(Ordered 7 banana)');
    const status = await page.$eval('#status', (el) => el.textContent);
    expect(status).toBe('Ordered 7 banana as a gift');
    const checked = await page.$eval('#gift', (el) => el.checked);
    expect(checked).toBeTruthy();
  });

  it('navigates', async () => {
    await page.click('aria/About us[role="link"]');
    await page.waitForFunction('location.pathname === "/about" && document.title === "About"');
    expect(await page.title()).toBe('About');
    await page.goBack();
    await page.waitForSelector('#email', { visible: true });
    await page.goForward();
    await page.waitForSelector('::-p-xpath(//h1[text()="About us"])');
    await page.reload();
    await page.keyboard.press('Tab');
    await page.mouse.click(10, 10);
  });

  it('keeps request interception as a TODO', async () => {
    await page.setRequestInterception(true);
    await expect(page).toMatchElement('h1');
  });
});
