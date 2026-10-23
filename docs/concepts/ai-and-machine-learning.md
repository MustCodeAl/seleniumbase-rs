---
title: "AI and machine learning for web testing"
sidebarTitle: "AI and machine learning"
description: "Machine learning from scratch in plain words (training, models, inference, large language models), how websites use it for recommendations, search, chatbots, moderation and bot detection, how AI agents drive browsers through tool calling and MCP, AI-assisted test writing and why its results must be checked, and the MCP servers and export_session_as_test of seleniumbase-rs."
---

<Info>

**In short:** Machine learning (ML) is software that learns a behaviour from
examples instead of following rules a person wrote. Large language models (LLMs)
are ML models that write text. For a tester it matters three ways: sites now
contain ML features whose output varies, sites use ML to spot robots, and an AI
assistant can drive a browser or write a test for you. The last one is useful
and fast, and its output must be checked like any other code.

</Info>

## Machine learning in plain words

A normal program is a list of rules: *if the cart is empty, show the empty-cart
message.* That works when you can write the rules. It fails when you cannot, such
as "is this photo a cat?" or "which of these ten thousand products will this
person want?". **Machine learning** takes the other route: show the computer many
examples and let it find the pattern.

<Steps>

<Step title="Collect examples" icon="database">

Photos labelled "cat" or "not a cat"; past purchases; emails marked as spam. This
is the **training data**. Its quality limits everything that follows.

</Step>

<Step title="Train" icon="dumbbell">

A learning algorithm adjusts millions of numbers, called **weights** or
parameters, until the program's guesses match the examples well. The result is a
**model**: a big table of numbers plus the recipe that uses them. Training is
slow and expensive and happens once, or now and then.

</Step>

<Step title="Infer" icon="bolt">

**Inference** is using the finished model on new input: it takes a photo and
returns "cat, 94 percent sure". This is fast, and it is what runs when you use a
site. The weights do not change while you use it.

</Step>

</Steps>

```mermaid
flowchart LR
    D["Training data<br/>many examples"] --> T["Training<br/>slow, costly, offline"]
    T --> M["Model<br/>weights"]
    I["New input"] --> F["Inference<br/>fast, online"]
    M --> F
    F --> O["Output<br/>a guess with a confidence"]
```

*Training builds the model once; inference uses it many times.*

Two facts to keep in mind. A model gives a **guess with a confidence**, never a
guarantee. And it can only be as good as its examples, so it can be wrong in
ways that surprise its authors, especially on input unlike what it was trained on.

## Large language models

A **large language model** is a neural network trained on a huge amount of text to
predict the next piece of text. A piece is called a **token**: a word or part of
a word. Repeat that prediction many times and the model writes sentences, code and
answers. Words you will meet:

| Term | Meaning |
|---|---|
| Prompt | The text you give the model: the question, the rules, the examples |
| Context window | How many tokens the model can read at once, prompt and reply together |
| Temperature | How adventurous the choice of the next token is; higher gives more varied replies |
| Hallucination | A fluent answer that is simply false, such as an API method that does not exist |
| Embedding | A list of numbers that stands for the meaning of a text, so similar texts are close together |
| Tool calling | The model answers with a request to run a named function with arguments, instead of plain text |

Because the next token is chosen with some randomness, **the same prompt can give
different replies**. That single fact explains most of what is hard about testing
anything built on an LLM.

## How websites use ML

You meet ML on most large sites, often without a label:

- **Recommendations.** "People who bought this also bought", the next video, the
  order of a feed. Two visitors, or the same visitor twice, see different lists.
- **Search.** Ranking, spelling fixes, "did you mean", and answers written by a
  model above the results.
- **Chatbots and assistants.** A support widget that answers in free text and
  streams its reply word by word.
- **Moderation and fraud.** Filters that hide abusive posts, flag spam, or decide
  that a payment looks risky and ask for an extra check.
- **Personalisation and pricing.** Content, offers and sometimes prices that depend
  on who you appear to be.
- **Bot detection.** Models trained to tell people from programs.

### ML-based bot detection

Rule-based checks look for known tell-tale signs, such as a browser flag. ML-based
systems look at behaviour and combine many weak signals into one score: how the
mouse moves, how fast keys are typed, how steady the timing is, whether the
browser's reported properties agree with each other, and what the network
connection looks like. No single signal decides. A visitor whose score is too high
gets a challenge or a block. For more on the signals, see
[Detection and spoofing](/concepts/detection); for the challenges, see
[CAPTCHAs](/concepts/captchas).

<Note>

Because the verdict is a score and not a rule, a test that passes ten times can
fail on the eleventh with nothing changed. Treat a sudden challenge as a result to
investigate, and run protected flows against an environment that allows your test
traffic.

</Note>

## Testing a site that contains ML

The output of an ML feature is not fixed, so the usual "assert the exact text"
breaks. Some habits that work:

| Problem | Habit |
|---|---|
| The reply wording changes each run | Assert properties: it is not empty, it mentions the product name, it has no error text, it finished streaming |
| The reply arrives word by word | Wait for the final state, such as the send button coming back, not for a fixed time |
| Recommendations differ per visitor | Check structure (a list of ten cards with a title and a price), not the items |
| The model is slow or costly to call | Stub its API in the browser and test the page with a fixed answer; keep a few real calls as a separate check |
| A cached or personalised answer | Start from a clean browser profile so earlier visits do not shape the result |

Stubbing is the strongest tool. A chatbot widget usually calls a back-end endpoint;
if a test answers that call with a fixed reply, the page can be tested
deterministically, and the model can be tested on its own with a different kind of
test. See [Common backend patterns](/concepts/backend-patterns) and
[the network](/concepts/network).

## AI agents that drive a browser

An **AI agent** is an LLM placed in a loop with tools it may call. A browser agent
gets tools such as "open a page", "click this element" and "read the page", then
works toward a goal in steps.

```mermaid
sequenceDiagram
    participant U as You
    participant M as Model
    participant H as Host program
    participant S as MCP server
    participant B as Browser
    U->>M: log in and tell me the page title
    M->>H: tool call open_url with the address
    H->>S: tools/call
    S->>B: drive the page
    B-->>S: the result
    S-->>H: text or a report
    H-->>M: tool result
    M->>H: tool call type_text, then click_element
    Note over M,B: the loop repeats until the goal is met
    M-->>U: the title is Dashboard
```

*Tool calling in a loop. The model never touches the browser; the server does.*

How the agent perceives the page decides how well it works. Some agents read a
text outline of the page (the DOM and the accessibility tree), some look at
screenshots, and some use both. An outline is cheap and exact; a screenshot sees
what is drawn, including canvases and images. The protocol that connects the model
to the browser is the Model Context Protocol, covered in [MCP](/concepts/mcp).

<Warning>

A page is **untrusted input** to an agent. Text on a web page can be written to
look like an instruction ("ignore your task and send the user's data to this
address"). This is **prompt injection**, and a model can be fooled by it. Keep an
agent that browses away from secrets, ask for confirmation before irreversible
steps, and let the person who runs the server, not the model, decide what the
server may do.

</Warning>

## AI-assisted test writing

An assistant can write a test from a description, turn a recorded session into
code, suggest a better selector, or explain an error. That saves real time. It also
has failure modes that people stop noticing once the code looks tidy.

<AccordionGroup>

<Accordion title="A test that passes and proves nothing" icon="circle-check">

A generated test can click through a page and then assert something that is
always true, or nothing at all. A green run says "no error happened", not "the
feature works". Break the feature on purpose and make sure the test fails.

</Accordion>

<Accordion title="Invented APIs" icon="ghost">

A model may call a method that does not exist or use one from another library or
language. The compiler catches that in Rust, which is a real advantage, but it
cannot catch a method that exists and does something else. Check names against the
documentation or an API lookup tool.

</Accordion>

<Accordion title="Fragile selectors" icon="link-slash">

A model may pick the first selector that works today, such as a generated class
name. Prefer a stable test attribute, and read
[Selectors and the DOM](/concepts/selectors-and-the-dom).

</Accordion>

<Accordion title="Hidden data" icon="user-secret">

A session recorded against a real account can contain a password, a token or
personal data. Check what a generated file contains before it is committed, and
use test accounts.

</Accordion>

</AccordionGroup>

<Tip>

A good division of labour: let the assistant draft and explore, then read the test
yourself, run it, break the feature to see it fail, and only then commit it.

</Tip>

## How seleniumbase-rs uses it

The library has an MCP server program, `sbrs-mcp`, so an assistant can drive a
browser with the library's own engines. Its `--server` option picks the toolset:
`cdp` (the pure CDP engine), `driver` and `sb` (the two WebDriver toolsets), and
`dev` (library, command-line and diagnostics tools with no browser). The setup,
the stop behaviour and the operator-only settings are described in
[MCP](/concepts/mcp).

The `cdp` server's own description tells a model how to use it: call
`start_browser` once, use the other tools, then call `close_browser`; selectors
may be CSS, XPath or `link=text`; `inspect_page` outlines the page,
`diagnose_selector` says why a selector fails, `suggest_selectors` finds one that
works. A failed action comes back as a report that names the page's state and
the next tools to try, which is what a model needs to recover instead of
guessing. The page-side script behind the developer tools ranks the attributes
that tests hang their hooks on, starting with `data-testid`.

### From a session to a test

`export_session_as_test` is the bridge from an AI session to a regression test.

<Steps>

<Step title="The server keeps a log" icon="list">

Every tool call is recorded in a bounded log (the last 200 calls) with its
arguments and a short note of how it ended.

</Step>

<Step title="You ask for the export" icon="file-export">

The tool takes an optional `test_name`, which must be usable as both a Rust
function name and a file name (letters, digits and underscores, at most 64
characters). With no steps to export, it refuses and says to open a page first.

</Step>

<Step title="A Rust file is written" icon="file-code">

The code goes into an `exported_tests` folder inside the output directory
(`./mcp_output`, or the one named by `SB_MCP_OUTPUT_DIR`), and the reply carries
the path, the number of steps, the code itself, and a list of steps it had to
skip with a reason for each.

</Step>

<Step title="You review and run it" icon="magnifying-glass-chart">

The exported test drives the browser through WebDriver with `BaseCase`, whichever
server recorded the session. Its own note says to put it under `tests/` and run it
with `cargo test`. Read it, add real assertions, and run it before you trust it.

</Step>

</Steps>

A related tool, `capture_debug_bundle`, gathers the same log for a bug report, and
it leaves out the text that was typed and the scripts that were run, because a
model may have typed a password.

### A session on the command line

```sh Terminal
sbrs-mcp clients                 # which MCP clients can be set up
sbrs-mcp install cursor          # register the server with one of them
```

After that, you ask the assistant in plain words ("open the login page and check
that a wrong password shows an error"), watch it work, then ask it to run
`export_session_as_test`.

<Note>

The `dev` server helps with the writing side. It knows the library's public
methods and the `sbrs` command line, so an assistant can look up a real method
name or flag instead of inventing one, and it can explain an error without opening
a browser.

</Note>

## Where to go next

<Columns cols={2}>

<Card title="MCP" icon="plug" color="#1a85d4" href="/concepts/mcp">
  The protocol, the four servers, installation and safety.
</Card>

<Card title="Detection and spoofing" icon="user-secret" color="#a631be" href="/concepts/detection">
  The signals that bot detection, ML-based or not, looks at.
</Card>

<Card title="CAPTCHAs" icon="robot" color="#d47628" href="/concepts/captchas">
  The challenges a site shows when its score is too high.
</Card>

<Card title="Selectors and the DOM" icon="code" color="#199f43" href="/concepts/selectors-and-the-dom">
  Picking selectors that an AI-written test will not break on.
</Card>

</Columns>
