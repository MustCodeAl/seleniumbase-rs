import re

import pytest
from playwright.sync_api import Page, expect

BASE = "http://127.0.0.1:4173/"


def test_title_and_heading(page: Page) -> None:
    page.goto(BASE)
    expect(page).to_have_title(re.compile("Shop"))
    expect(page).to_have_url(BASE)
    expect(page.get_by_role("heading", name="Welcome to the shop")).to_be_visible()
    assert page.title() == "Shop Fixture"
    assert "127.0.0.1" in page.url


def test_order_form(page: Page) -> None:
    page.goto(BASE)
    page.get_by_label("Email").fill("py@example.test")
    expect(page.get_by_label("Email")).to_have_value("py@example.test")
    page.locator("#qty").fill("4")
    page.get_by_label("Fruit").select_option(label="Apple")
    page.get_by_label("Fruit").select_option(value="cherry")
    page.get_by_role("checkbox", name="Gift wrap").set_checked(True)
    expect(page.locator("#gift")).to_be_checked()
    expect(page.get_by_test_id("place-order")).to_be_enabled()
    expect(page.locator("#disabled")).not_to_be_enabled()
    page.get_by_role("button", name="Place order", exact=True).click()
    expect(page.locator("#status")).to_contain_text("Ordered 4 cherry")
    status = page.locator("#status").text_content()
    assert status == "Ordered 4 cherry as a gift"


def test_lists_and_reads(page: Page) -> None:
    page.goto(BASE)
    expect(page.locator("#items li")).to_have_count(3)
    expect(page.locator("li").first).to_have_text("Apple")
    expect(page.locator("li").last).to_have_text("Cherry")
    expect(page.locator("li").filter(has_text="Banana")).to_be_visible()
    expect(page.get_by_text("Fresh fruit")).to_be_visible()
    expect(page.get_by_alt_text("Shop logo")).to_be_visible()
    expect(page.get_by_title("About the shop")).to_have_attribute("href", "/about")
    href = page.get_by_role("link").get_attribute("href")
    assert href == "/about"
    count = page.locator("li").count()
    assert count == 3
    shown = page.locator("#intro").is_visible()
    assert shown
    value = page.locator("#qty").input_value()
    assert value == "1"
    total = page.evaluate("() => 6 * 7")
    assert total == 42


def test_navigation_and_keyboard(page: Page) -> None:
    page.goto(BASE)
    page.get_by_role("link", name="About us").click()
    page.wait_for_url("**/about")
    expect(page).to_have_url(re.compile(r"/about$"))
    page.go_back()
    page.wait_for_load_state()
    page.locator("#email").click()
    page.keyboard.type("keys@example.test")
    page.keyboard.press("Enter")
    expect(page).to_have_title("Ordered")
    page.mouse.click(5, 5)
    page.wait_for_timeout(50)


@pytest.mark.skip(reason="kept skipped")
def test_skipped(page: Page) -> None:
    page.goto(BASE)


def test_with_a_todo(page: Page) -> None:
    page.goto(BASE)
    with page.expect_popup() as popup_info:
        page.get_by_text("About us").click()
    expect(page.locator("h1")).to_be_visible()
