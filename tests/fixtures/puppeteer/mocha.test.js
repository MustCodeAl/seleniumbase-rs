// A Mocha test with chai and node's assert, against the loopback shop page.
const puppeteer = require('puppeteer');
const { expect } = require('chai');
const assert = require('assert');

const BASE = 'http://127.0.0.1:4173/';

describe('shop (mocha)', function () {
  this.timeout(30000);
  let browser;
  let page;

  before(async () => {
    browser = await puppeteer.launch();
    page = await browser.newPage();
  });

  beforeEach(async () => {
    await page.goto(BASE);
  });

  after(async () => {
    await browser.close();
  });

  it('reads the page', async () => {
    const title = await page.title();
    expect(title).to.equal('Shop Fixture');
    assert.strictEqual(title, 'Shop Fixture');
    const intro = await page.$eval('#intro', el => el.innerText);
    expect(intro).to.include('fruit');
    expect(intro).to.have.lengthOf(21);
    const count = await page.$$eval('#items li', els => els.length);
    assert.equal(count, 3);
    expect(count).to.be.above(2);
    const checked = await page.$eval('#gift', el => el.checked);
    expect(checked).to.be.false;
  });

  it('submits the order', async () => {
    await page.type('#email', 'mocha@example.test');
    await page.click('#gift');
    await page.select('#fruit', 'apple');
    await page.click('#submit');
    await page.waitForSelector('::-p-text(Ordered 1 apple)');
    const status = await page.$eval('#status', el => el.textContent.trim());
    assert.strictEqual(status, 'Ordered 1 apple as a gift');
    expect(status).to.not.equal('');
  });
});
