"""A complete end-to-end test for an e-commerce website."""
from seleniumbase import BaseCase
BaseCase.main(__name__, __file__)


class MyTestClass(BaseCase):
    def test_swag_labs(self):
        self.goto("https://www.saucedemo.com")
        self.type("#user-name", "standard_user")
        self.type("#password", "secret_sauce\n")
        self.assert_element("div.inventory_list")
        self.assert_exact_text("Products", "span.title")
        self.click('button[name*="backpack"]')
        self.click("#shopping_cart_container a")
        self.assert_exact_text("Your Cart", "span.title")
        self.assert_text("Backpack", "div.cart_item")
        self.click("button#checkout")
        self.type("#first-name", "SeleniumBase")
        self.type("#last-name", "Automation")
        self.type("#postal-code", "77123")
        self.click("input#continue")
        self.assert_text("Checkout: Overview")
        self.assert_text("Backpack", "div.cart_item")
        self.assert_text("29.99", "div.inventory_item_price")
        self.click("button#finish")
        self.assert_exact_text("Thank you for your order!", "h2")
        self.assert_element('img[alt="Pony Express"]')
        self.js_click("a#logout_sidebar_link")
        self.assert_element("div#login_button_container")


#######################################################################
#
#    ****  NOTES / USEFUL INFO  ****
#
# 1. By default, page elements are identified by "css selector".
#
#    XPath is used by default if the arg starts with "/", "./", or "(":
#    [
#        self.click('/html/body/div[3]/div[4]/p[2]/a')
#    ]
#
# 2. Most methods have the optional "timeout" argument.
#    Here's an example of changing the "timeout":
#    [
#        self.assert_element('img[alt="Python"]', timeout=15)
#    ]
#
# 3. SeleniumBase methods often perform multiple actions.
#    Example: self.type(SELECTOR, TEXT) does the following:
#    * Waits for the element to be visible
#    * Clears the text field
#    * Types in the new text
#    * Presses Enter/Return if the text ends in "\n": element.submit()
#
# 4. There are duplicate method names that exist for the same method:
#    Example:
#    self.goto() = self.open() = self.visit()
#    self.type() = self.update_text() = self.fill()
#
# (The upstream file goes on with more notes; they are comments only.)
