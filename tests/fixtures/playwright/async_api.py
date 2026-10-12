import asyncio

from playwright.async_api import async_playwright, expect


async def main() -> None:
    async with async_playwright() as p:
        browser = await p.chromium.launch()
        page = await browser.new_page()
        await page.goto("http://127.0.0.1:4173/")
        await page.locator("#email").fill("async@example.test")
        await page.locator("#qty").fill("5")
        await page.locator("#gift").check()
        await page.get_by_text("Place order").click()
        await expect(page.locator("#status")).to_have_text("Ordered 5 apple as a gift")
        title = await page.title()
        assert title == "Ordered"
        heading = await page.locator("h1").inner_text()
        print(heading)
        await page.screenshot(path="async.png")
        await browser.close()


asyncio.run(main())
