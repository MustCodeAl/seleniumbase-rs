Feature: A counter
  Scenario: Adding to the counter
    Given the counter holds 2
    When I add 3
    Then the counter holds 5

  Scenario: Adding twice
    Given the counter holds 0
    When I add 1 twice
    Then the counter holds 2
