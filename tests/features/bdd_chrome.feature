Feature: A page in a real Chrome

  Scenario: The title of a page
    Given I open a page titled "Hello BDD"
    Then the title is "Hello BDD"

  Scenario: The wrong title
    Given I open a page titled "Hello BDD"
    Then the title is "Goodbye"
