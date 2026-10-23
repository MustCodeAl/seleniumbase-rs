Feature: The shop, on a mocked browser

  Background:
    Given the shop page is open

  Scenario: The page answers
    When I read the address of the page
    Then the address is "https://shop.test/cart"

  Scenario: Items in a table
    Given these items are in the cart:
      | name  | price |
      | boots | 40    |
      | socks | 5     |
    Then the cart holds 2 items

  Scenario Outline: Counting
    When I count <n> items
    Then the count is <n>

    Examples:
      | n |
      | 1 |
      | 2 |

  Scenario: A step fails
    When I read the address of the page
    Then the address is "https://nowhere.test"
