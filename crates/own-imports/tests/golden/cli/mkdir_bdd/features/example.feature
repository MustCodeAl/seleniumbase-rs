Feature: Example Domain

  Scenario: Open a page
    Given Open "https://example.com"
    Then Assert title "Example Domain"
    And Assert text "This domain is for use in" in "p"
