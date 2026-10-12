// Its check cannot hold: tests/puppeteer_import_chrome.rs runs the generated
// test and checks that it fails, which shows the converted checks really
// check the page.
describe('a check that fails', () => {
  it('expects a title the page does not have', async () => {
    await page.goto('http://127.0.0.1:4173/');
    const title = await page.title();
    expect(title).toBe('Not the title');
  });
});
