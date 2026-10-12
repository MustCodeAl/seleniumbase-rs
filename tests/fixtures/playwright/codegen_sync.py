import re
from playwright.sync_api import Playwright, sync_playwright, expect


def run(playwright: Playwright) -> None:
    browser = playwright.chromium.launch(headless=False)
    context = browser.new_context()
    page = context.new_page()
    page.goto("http://127.0.0.1:4173/")
    page.get_by_placeholder("you@example.test").click()
    page.get_by_placeholder("you@example.test").fill("sync@example.test")
    page.get_by_label("Fruit").select_option("banana")
    page.get_by_role("button", name="Place order").click()
    expect(page.locator("#status")).to_have_text("Ordered 1 banana")
    page.get_by_role("link", name="About us").click()
    expect(page.get_by_role("heading")).to_have_text("About us")

    # ---------------------
    context.close()
    browser.close()


with sync_playwright() as playwright:
    run(playwright)
