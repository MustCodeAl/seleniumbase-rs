Feature: The built-in steps on a real Chrome

  Scenario: A small form
    Given Open "data:text/html,<title>Form</title><script>function go(){document.getElementById('out').textContent=document.getElementById('name').value}</script><input id=name><button id=go onclick=go()>Go</button><p id=out></p>"
    When Type "Ada Lovelace" into "#name"
    And In '#name' add ' and Bob'
    And Click "#go"
    Then Assert text "Ada Lovelace and Bob" in "#out"
    And Text in '#out' should be 'Ada Lovelace and Bob'
    And Assert title "Form"
    And Assert element "#go"
