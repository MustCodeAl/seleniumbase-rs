Feature: The built-in steps, on a mock WebDriver

  Scenario: Navigation
    Given Open "https://shop.test/start"
    When User goes to URL 'https://shop.test/next'
    And Refresh page
    And Go back
    And Go forward
    And Open new tab
    And Accept alert
    And Dismiss alert
    And Clear all cookies
    And Maximize window
    Then Assert URL "https://shop.test/next"
    But The URL should contain "shop.test"
    And the driver received "POST /url" 2 times
    And the driver received "POST /refresh" 1 times
    And the driver received "POST /back" 1 times
    And the driver received "POST /forward" 1 times
    And the driver received "POST /execute/sync" with "window.open"
    And the driver received "POST /alert/accept" 1 times
    And the driver received "POST /alert/dismiss" 1 times
    And the driver received "DELETE /cookie" 1 times
    And the driver received "POST /window/maximize" 1 times
    And the driver received "POST /url" with "https://shop.test/next"

  Scenario: Elements
    Given Open "https://shop.test/form"
    When Type text "demo" into "#user"
    And In '#note' type 'it\'s "fine"'
    And Type "tab\there\nand a \"quote\"" into "#note"
    And Add "more" in "#note"
    And Click 'button#go'
    And Double click "#go"
    And Hover "#menu"
    And Right click "#menu"
    And Clear text in "#user"
    Then Assert element "#go"
    And the driver received "POST /element/e1/click" at least 1 times
    And the driver received "POST /element/e1/clear" at least 1 times
    And the driver received "POST /element/e1/value" with "demo"
    And the driver received "POST /element/e1/value" with "it's \"fine\""
    And the driver received "POST /element/e1/value" with "tab\there\nand a \"quote\""
    And the driver received "POST /element/e1/value" with "more"
    And the driver received "POST /actions" at least 3 times

  Scenario: Asserts
    Given Open "https://shop.test/"
    Then Assert title "Mock Shop"
    And The title should contain "Shop"
    And Assert text "Hello" in "#msg"
    And Text in '#msg' should be 'Hello World'
    And Assert text 'World'
    And Assert element present "#msg"
    And Assert element not visible "#hidden"
    And Text "Nothing here" should not be visible
    And Wait for "#msg" to be visible
    And Wait for text "Hello" in "#msg"
    And Assert non-empty text in "#msg"
    And In "#msg" assert attribute "class"
    And In "#msg" assert attribute/value "class"/"btn"
    And Assert link text "Next"

  Scenario: A wrong title fails
    Given Open "https://shop.test/"
    Then Assert title "Another Shop"

  Scenario: Deferred asserts
    Given Open "https://shop.test/"
    When Deferred assert element "#msg"
    And Deferred assert element present "#msg"
    And Deferred assert text 'Hello' in '#msg'
    And Deferred assert text "Hello World"
    And Deferred assert exact text "Hello World" in "#msg"
    And Deferred assert non-empty text in "#msg"
    Then Process deferred asserts

  Scenario: A deferred assert fails at the end
    Given Open "https://shop.test/"
    When Deferred assert exact text "Goodbye" in "#msg"
    Then Process deferred asserts

  Scenario: Frames
    Given Open "https://shop.test/frames"
    When Switch to iframe "#inner"
    And User switches to frame '#deeper'
    And Switch to parent frame
    And Exit from iframes
    Then the driver received "POST /frame" 3 times
    And the driver received "POST /frame/parent" 1 times

  Scenario: Pages saved and sleeping
    Given Open "https://shop.test/"
    When Sleep for 0.01 seconds
    And Wait for 0.01 seconds
    And Save screenshot to logs
    And Save page source to logs
    Then the driver received "GET /screenshot" 1 times
    And the driver received "GET /source" 1 times
