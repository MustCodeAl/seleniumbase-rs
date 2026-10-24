Feature: Example Domain

  Scenario: Open a page
    Given Open "https://example.com"
    Then Assert title "Example Domain"
    And Assert text "Example Domain" in "h1"
