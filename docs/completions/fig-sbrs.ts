const completionSpec: Fig.Spec = {
  name: "sbrs",
  description: "SeleniumBaseRust CLI",
  subcommands: [
    {
      name: "version",
      description: "Print the sbrs version, as `sbrs --version` does.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "open",
      description: "Open a URL in the browser.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "url",
          description: "URL to open in the browser.",
        },
      ],
    },
    {
      name: "smoke",
      description: "Run a smoke test against a URL.",
      options: [
        {
          name: "--title-contains",
          description: "Expected substring in the page title",
          args: {
            name: "title_contains",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "url",
          description: "URL to open or smoke-test",
        },
      ],
    },
    {
      name: "cdp",
      description: "Execute a Chrome DevTools Protocol command.",
      options: [
        {
          name: "--cmd",
          description: "CDP method name (e.g. Runtime.evaluate)",
          isRequired: true,
          args: {
            name: "cmd",
          },
        },
        {
          name: "--params",
          description: "JSON object of CDP parameters",
          args: {
            name: "params",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "cache-clear",
      description: "Clear the browser cache via CDP.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "screenshot",
      description: "Capture a screenshot of the current page.",
      options: [
        {
          name: "--path",
          description: "Output image file path (default: a new file in latest_logs/)",
          args: {
            name: "path",
            template: "filepaths",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "save-source",
      description: "Save the current page source to a file.",
      options: [
        {
          name: "--path",
          description: "Output HTML file path (default: a new file in latest_logs/)",
          args: {
            name: "path",
            template: "filepaths",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "assert-element",
      description: "Assert that an element exists.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "wait-for-text",
      description: "Wait until an element contains the expected text.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--text",
          description: "Expected text substring to wait for",
          isRequired: true,
          args: {
            name: "text",
          },
        },
        {
          name: "--timeout",
          description: "Maximum wait time in seconds",
          args: {
            name: "timeout",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "hover",
      description: "Hover over an element.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "hover-and-click",
      description: "Hover over one element, then click another.",
      options: [
        {
          name: "--hover-css",
          description: "CSS selector to hover over",
          isRequired: true,
          args: {
            name: "hover_css",
          },
        },
        {
          name: "--click-css",
          description: "CSS selector to click",
          isRequired: true,
          args: {
            name: "click_css",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "select-option",
      description: "Select an option from a dropdown.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--text",
          description: "Visible text of the option to select",
          args: {
            name: "text",
          },
        },
        {
          name: "--value",
          description: "Option value attribute",
          args: {
            name: "value",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "drag-and-drop",
      description: "Drag one element onto another.",
      options: [
        {
          name: "--source-css",
          description: "CSS selector of the element to drag",
          isRequired: true,
          args: {
            name: "source_css",
          },
        },
        {
          name: "--target-css",
          description: "CSS selector of the drop target",
          isRequired: true,
          args: {
            name: "target_css",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "cdp-click-element",
      description: "Click an element using CDP.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "cdp-type-text",
      description: "Type text using CDP input injection.",
      options: [
        {
          name: "--text",
          description: "Text to inject via CDP",
          isRequired: true,
          args: {
            name: "text",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "get-text",
      description: "Print the visible text of an element.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "get-attribute",
      description: "Print an element attribute value.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--attribute",
          description: "Attribute name to read or assert",
          isRequired: true,
          args: {
            name: "attribute",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "get-property",
      description: "Print an element property value.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--property",
          description: "Property name to read",
          isRequired: true,
          args: {
            name: "property",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "get-title",
      description: "Print the page title.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "get-current-url",
      description: "Print the current URL.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "clear-cookies",
      description: "Delete all browser cookies.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "accept-alert",
      description: "Accept the alert that the page opens when it loads.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "dismiss-alert",
      description: "Dismiss the alert that the page opens when it loads.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "get-alert-text",
      description: "Print the text of the alert that the page opens when it loads.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "type-alert-text",
      description: "Type text into the prompt that the page opens when it loads.",
      options: [
        {
          name: "--text",
          description: "Text to type into the alert prompt",
          isRequired: true,
          args: {
            name: "text",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "clear-local-storage",
      description: "Clear the page local storage.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "get-local-storage-item",
      description: "Print a local storage value by key.",
      options: [
        {
          name: "--key",
          description: "Storage key",
          isRequired: true,
          args: {
            name: "key",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "set-local-storage-item",
      description: "Set a local storage key/value pair.",
      options: [
        {
          name: "--key",
          description: "Storage key",
          isRequired: true,
          args: {
            name: "key",
          },
        },
        {
          name: "--value",
          description: "Value to store in local storage",
          isRequired: true,
          args: {
            name: "value",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "remove-local-storage-item",
      description: "Remove a local storage entry by key.",
      options: [
        {
          name: "--key",
          description: "Storage key",
          isRequired: true,
          args: {
            name: "key",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "get-cookies",
      description: "Print all browser cookies as JSON.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "patch-chromedriver",
      description: "Patch a chromedriver binary to reduce detection surface.",
      options: [
        {
          name: "--path",
          description: "Path to the chromedriver binary",
          isRequired: true,
          args: {
            name: "path",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "patch-chrome",
      description: "Patch a Chrome/Chromium binary for native-level spoofing.",
      options: [
        {
          name: "--path",
          description: "Path to the Chrome/Chromium binary",
          isRequired: true,
          args: {
            name: "path",
          },
        },
        {
          name: "--cache-dir",
          description: "Directory where patched binaries are cached",
          args: {
            name: "cache_dir",
            template: "folders",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "proxy",
      description: "Run a small forward proxy on this machine (HTTP and CONNECT, no authentication).",
      options: [
        {
          name: "--hostname",
          description: "The address to listen on. Anything but a loopback address makes this an open proxy for other machines, and the command says so.",
          args: {
            name: "host",
          },
        },
        {
          name: "--port",
          description: "The port to listen on; 0 picks a free one.",
          args: {
            name: "port",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "download",
      description: "Download Selenium Server 4, the jar that `grid-hub` and `grid-node` run.",
      subcommands: [
        {
          name: "server",
          description: "Download the Selenium Server 4 jar that `grid-hub` and `grid-node` run.",
          options: [
            {
              name: "--version",
              description: "The release to download, such as 4.51.0 (default: the version this build pins).",
              args: {
                name: "version",
              },
            },
            {
              name: ["-h", "--help"],
              description: "Print help",
            },
          ],
        },
      ],
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: ["encrypt", "obfuscate"],
      description: "Encrypt text with a passphrase (AES-256-GCM); read from stdin if TEXT is omitted.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "text",
          description: "The text to encrypt.",
          isOptional: true,
        },
      ],
    },
    {
      name: ["decrypt", "unobfuscate"],
      description: "Decrypt a token made by `sbrs encrypt`; read from stdin if TOKEN is omitted. Python's `sbrs unobfuscate` is the same command.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "token",
          description: "The token to decrypt.",
          isOptional: true,
        },
      ],
    },
    {
      name: "doctor",
      description: "Run a diagnostic check on the environment and configuration.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "prompt",
      description: "Print one short line for a shell prompt: the installed Chrome's major version and whether a matching chromedriver is at hand.",
      options: [
        {
          name: "--format",
          description: "The line to print; placeholders {browser} {major} {driver} {mark} {driver_major}, {{ and }} for braces",
          args: {
            name: "template",
          },
        },
        {
          name: "--ascii",
          description: "Mark the driver with + - ? instead of ✓ ✗ ?",
        },
        {
          name: "--browser-path",
          description: "Browser to report on (default: SB_CHROME_BIN, else the installed Chrome or Chromium)",
          args: {
            name: "path",
            template: "filepaths",
          },
        },
        {
          name: "--no-cache",
          description: "Run --version again instead of using the versions cached in the user cache folder",
        },
        {
          name: "--verbose",
          description: "Explain on standard error what was found and how",
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "assert-text-visible",
      description: "Assert that an element contains the expected text.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--text",
          description: "Expected visible text",
          isRequired: true,
          args: {
            name: "text",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "assert-text-not-visible",
      description: "Assert that an element does not contain the given text.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--text",
          description: "Text that should not be visible",
          isRequired: true,
          args: {
            name: "text",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "assert-attribute",
      description: "Assert an element attribute equals an expected value.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--attribute",
          description: "Attribute name to read or assert",
          isRequired: true,
          args: {
            name: "attribute",
          },
        },
        {
          name: "--value",
          description: "Expected attribute value",
          isRequired: true,
          args: {
            name: "value",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "assert-title",
      description: "Assert the page title contains the expected text.",
      options: [
        {
          name: "--text",
          description: "Expected page title substring",
          isRequired: true,
          args: {
            name: "text",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "wait-for-ready-state-complete",
      description: "Wait until the document readyState is complete.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "get-window-position",
      description: "Print the browser window position.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "is-element-visible",
      description: "Print whether an element is visible.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "is-text-visible",
      description: "Print whether the expected text is visible in an element.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--text",
          description: "Expected text to check",
          isRequired: true,
          args: {
            name: "text",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "wait-for-element-not-visible",
      description: "Wait until an element is no longer visible.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--timeout",
          description: "Maximum wait time in seconds",
          args: {
            name: "timeout",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "save-cookies",
      description: "Save browser cookies to a JSON file.",
      options: [
        {
          name: "--file",
          description: "JSON file to save cookies to",
          isRequired: true,
          args: {
            name: "file",
            template: "filepaths",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "load-cookies",
      description: "Load browser cookies from a JSON file.",
      options: [
        {
          name: "--file",
          description: "JSON file to load cookies from",
          isRequired: true,
          args: {
            name: "file",
            template: "filepaths",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "highlight-click",
      description: "Highlight an element, then click it.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "is-checked",
      description: "Print whether a checkbox is checked.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "check-if-unchecked",
      description: "Check a checkbox only if it is unchecked.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "uncheck-if-checked",
      description: "Uncheck a checkbox only if it is checked.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "wait-for-element-present",
      description: "Wait until an element is present in the DOM.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--timeout",
          description: "Maximum wait time in seconds",
          args: {
            name: "timeout",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "add-text",
      description: "Append text to an input or textarea.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--text",
          description: "Text to append to the element",
          isRequired: true,
          args: {
            name: "text",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "send-keys",
      description: "Send keystrokes to an element.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--text",
          description: "Keys to send to the element",
          isRequired: true,
          args: {
            name: "text",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "get-value",
      description: "Print the value of a form element.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "click-visible-elements",
      description: "Click all visible elements matching a selector.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "wait-for-and-accept-alert",
      description: "Wait for an alert that the page opens, then accept it.",
      options: [
        {
          name: "--timeout",
          description: "Maximum wait time in seconds",
          args: {
            name: "timeout",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "wait-for-and-dismiss-alert",
      description: "Wait for an alert that the page opens, then dismiss it.",
      options: [
        {
          name: "--timeout",
          description: "Maximum wait time in seconds",
          args: {
            name: "timeout",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "is-link-text-visible",
      description: "Print whether a link with the given text is visible.",
      options: [
        {
          name: "--text",
          description: "Exact link text to check",
          isRequired: true,
          args: {
            name: "text",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "is-partial-link-text-visible",
      description: "Print whether a link containing the given text is visible.",
      options: [
        {
          name: "--text",
          description: "Link text substring to check",
          isRequired: true,
          args: {
            name: "text",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "assert-link-text",
      description: "Assert a link with the given text is visible.",
      options: [
        {
          name: "--text",
          description: "Exact link text to assert",
          isRequired: true,
          args: {
            name: "text",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "click-partial-link-text",
      description: "Click a link containing the given text.",
      options: [
        {
          name: "--text",
          description: "Link text substring to click",
          isRequired: true,
          args: {
            name: "text",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "human-type",
      description: "Type text with human-like timing and noise.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--text",
          description: "Text to type with humanized timing",
          isRequired: true,
          args: {
            name: "text",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "human-click",
      description: "Click an element with human-like timing and noise.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "smooth-scroll-to",
      description: "Smoothly scroll an element into view.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "uc-click",
      description: "Perform a UC-mode click on an element.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "uc-type",
      description: "Perform UC-mode typing into an element.",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRequired: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--text",
          description: "Text to type in UC mode",
          isRequired: true,
          args: {
            name: "text",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: ["install", "get"],
      description: "Download a driver or a browser into ./downloaded_drivers/ (Python's `sbrs get`).",
      options: [
        {
          name: ["-p", "--path"],
          description: "Also copy the driver into the folder named by SB_DRIVER_PATH_DIR (else /usr/local/bin). Never uses sudo and never replaces a symbolic link. Drivers only.",
        },
        {
          name: "--revision",
          description: "The Chromium snapshot to take (chromium only); the newest when left out.",
          args: {
            name: "revision",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "driver",
          description: "The program: chromedriver, uc_driver, geckodriver (or firefoxdriver), edgedriver (or msedgedriver), or the browsers cft, chs and chromium.",
          suggestions: ["chromedriver", "geckodriver", "edgedriver", "uc_driver", "cft", "chs", "chromium"],
        },
        {
          name: "version",
          description: "A version word (stable, beta, dev, canary, latest, previous, mlatest, browser, keep), a major version, a full build, or for geckodriver a version such as 0.36.0.",
          isOptional: true,
        },
      ],
    },
    {
      name: "test",
      description: "Run the project's tests, or one example, through Cargo or cargo-nextest.",
      options: [
        {
          name: "--test",
          description: "Run one integration test file from tests/.",
          args: {
            name: "name",
          },
        },
        {
          name: "--example",
          description: "Run one example from examples/ instead of the tests.",
          args: {
            name: "name",
          },
        },
        {
          name: "--release",
          description: "Build with the release profile.",
        },
        {
          name: "--features",
          description: "Cargo features to enable, separated by commas.",
          isRepeatable: true,
          args: {
            name: "features",
          },
        },
        {
          name: "-k",
          description: "Select tests by name, as pytest's -k: `login or signup`, `not slow`, `(a or b) and not c`.",
          args: {
            name: "expr",
          },
        },
        {
          name: "-m",
          description: "Select tests by marker, as pytest's -m. A marker is a module name: `-m smoke` runs the tests in `mod smoke`.",
          args: {
            name: "expr",
          },
        },
        {
          name: ["-n", "--threads"],
          description: "How many tests run at once, as pytest-xdist's -n: a number or `auto`.",
          args: {
            name: "n|auto",
          },
        },
        {
          name: ["-s", "--no-capture", "--nocapture"],
          description: "Show what the tests print as they run (pytest's -s).",
        },
        {
          name: ["--co", "--collect-only"],
          description: "List the tests that would run and how many (`collected N items`) without running them.",
        },
        {
          name: "--all-scripts",
          description: "After the tests, run every example of the packages with `cargo run --example`.",
        },
        {
          name: "--browser",
          description: "The browser to run: chrome, edge, firefox, brave, ...",
          args: {
            name: "browser",
            suggestions: ["chrome", "chromium", "edge", "firefox", "brave", "comet", "opera", "safari", "ie", "cft", "chs"],
          },
        },
        {
          name: ["--headless", "--headless2"],
          description: "Run without a visible window (Chrome's new headless mode)",
          args: {
            name: "headless",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--binary-location", "--bl"],
          description: "The browser binary to run",
          args: {
            name: "path",
            template: "filepaths",
          },
        },
        {
          name: ["--proxy", "--proxy-server", "--proxy-string"],
          description: "The proxy server: HOST:PORT or USER:PASS@HOST:PORT",
          args: {
            name: "value",
          },
        },
        {
          name: "--user-data-dir",
          description: "The browser profile folder",
          args: {
            name: "dir",
            template: "folders",
          },
        },
        {
          name: "--extension-dir",
          description: "Unpacked extension folders to load, separated by commas",
          args: {
            name: "dir",
            template: "folders",
          },
        },
        {
          name: "--locale",
          description: "The browser locale",
          args: {
            name: "value",
          },
        },
        {
          name: ["--user-agent", "--agent"],
          description: "The user agent string",
          args: {
            name: "value",
          },
        },
        {
          name: "--mode",
          description: "The driver mode: webdriver, cdp or uc",
          args: {
            name: "mode",
            suggestions: ["webdriver", "cdp", "uc"],
          },
        },
        {
          name: ["--ad-block", "--adblock", "--block-ads"],
          description: "Block the requests of ad and tracker hosts",
          args: {
            name: "ad_block",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--reuse-session", "--rs"],
          description: "Share browsers among the tests of the run",
          args: {
            name: "reuse_session",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--reuse-class-session", "--rcs"],
          description: "Share a browser among the tests of one module",
          args: {
            name: "reuse_class_session",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--mobile", "--mobile-emulator"],
          description: "Emulate a mobile phone",
          args: {
            name: "mobile",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--proxy-pac-url", "--pac-url"],
          description: "A proxy auto-config URL",
          args: {
            name: "url",
          },
        },
        {
          name: ["--esc-end", "--ee"],
          description: "End the current test when ESC is pressed in the browser",
          args: {
            name: "esc_end",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--rec-sb-cdp",
          description: "Write recordings as pure CDP code",
          args: {
            name: "rec_sb_cdp",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--fingerprint",
          description: "A fingerprint file every browser presents",
          args: {
            name: "path",
            template: "filepaths",
          },
        },
        {
          name: ["--start-page", "--url"],
          description: "The page every browser opens first",
          args: {
            name: "url",
          },
        },
        {
          name: ["--settings-file", "--settings"],
          description: "The settings file to read instead of sbase_config.toml",
          args: {
            name: "path",
            template: "filepaths",
          },
        },
        {
          name: "--chrome",
          description: "Run Chrome",
          args: {
            name: "chrome",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--edge",
          description: "Run Edge",
          args: {
            name: "edge",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--firefox",
          description: "Run Firefox",
          args: {
            name: "firefox",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--brave",
          description: "Run Brave",
          args: {
            name: "brave",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--comet",
          description: "Run Comet",
          args: {
            name: "comet",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--opera",
          description: "Run Opera",
          args: {
            name: "opera",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--safari",
          description: "Run Safari",
          args: {
            name: "safari",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--ie",
          description: "Run Internet Explorer",
          args: {
            name: "ie",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--cft",
          description: "Run Chrome for Testing",
          args: {
            name: "cft",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--chs",
          description: "Run chrome-headless-shell",
          args: {
            name: "chs",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--chromium", "--use-chromium"],
          description: "Run Chromium",
          args: {
            name: "chromium",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--headed", "--gui"],
          description: "Run with a visible window",
          args: {
            name: "headed",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--uc", "--undetected", "--undetectable"],
          description: "Run in undetected-chromedriver mode",
          args: {
            name: "uc",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--headless1",
          description: "Run headless with chrome-headless-shell (Chrome's old headless mode)",
          args: {
            name: "headless1",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--locale-code",
          description: "The browser language code, such as fr or ar_eg",
          args: {
            name: "value",
          },
        },
        {
          name: "--server",
          description: "The Selenium Grid server to connect to",
          args: {
            name: "value",
          },
        },
        {
          name: "--port",
          description: "The port of the Selenium Grid server",
          args: {
            name: "value",
          },
        },
        {
          name: "--protocol",
          description: "The protocol of the Selenium Grid server: http or https",
          args: {
            name: "value",
          },
        },
        {
          name: "--chromium-arg",
          description: "Extra Chromium switches, separated by commas",
          args: {
            name: "value",
          },
        },
        {
          name: ["--dark-mode", "--dark"],
          description: "Force Chrome's dark theme on web pages",
          args: {
            name: "dark_mode",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--devtools", "--open-devtools"],
          description: "Open the developer tools with each tab (headed only)",
          args: {
            name: "devtools",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--disable-features",
          description: "Chromium features to switch off, separated by commas",
          args: {
            name: "value",
          },
        },
        {
          name: "--disable-gpu",
          description: "Switch the GPU off",
          args: {
            name: "disable_gpu",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--disable-ws", "--dws", "--disable-web-security"],
          description: "Switch web security off",
          args: {
            name: "disable_ws",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--guest-mode", "--guest"],
          description: "Open the browser as a guest",
          args: {
            name: "guest_mode",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--incognito", "--incognito-mode"],
          description: "Open the browser in an incognito window",
          args: {
            name: "incognito",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--host-resolver-rules",
          description: "Chromium rules that map host names to addresses",
          args: {
            name: "value",
          },
        },
        {
          name: "--no-sandbox",
          description: "Start Chromium without its sandbox",
          args: {
            name: "no_sandbox",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--swiftshader",
          description: "Render with SwiftShader, a software WebGL renderer",
          args: {
            name: "swiftshader",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--enable-3d-apis",
          description: "Keep the GPU on in a headless session, for WebGL",
          args: {
            name: "enable_3d_apis",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--maximize", "--maximize-window", "--fullscreen"],
          description: "Maximize the browser window",
          args: {
            name: "maximize",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--block-images",
          description: "Do not load images",
          args: {
            name: "block_images",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--disable-js",
          description: "Turn JavaScript off",
          args: {
            name: "disable_js",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--disable-cookies",
          description: "Refuse every cookie",
          args: {
            name: "disable_cookies",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--do-not-track",
          description: "Send the DNT: 1 header",
          args: {
            name: "do_not_track",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--external-pdf",
          description: "Download PDF files instead of showing them",
          args: {
            name: "external_pdf",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--enable-sync",
          description: "Leave Chrome Sync on",
          args: {
            name: "enable_sync",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--disable-csp", "--dcsp", "--no-csp"],
          description: "Strip the Content Security Policy from every response",
          args: {
            name: "disable_csp",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--use-auto-ext", "--auto-ext"],
          description: "Keep Chrome's automation extension",
          args: {
            name: "use_auto_ext",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--proxy-driver",
          description: "Download the driver through the proxy",
          args: {
            name: "proxy_driver",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--page-load-strategy", "--pls"],
          description: "How long a navigation waits: normal, eager or none",
          args: {
            name: "strategy",
            suggestions: ["normal", "eager", "none"],
          },
        },
        {
          name: "--window-size",
          description: "The window size: WIDTH,HEIGHT",
          args: {
            name: "value",
          },
        },
        {
          name: "--window-position",
          description: "The window position: X,Y",
          args: {
            name: "value",
          },
        },
        {
          name: ["--device-metrics", "--metrics"],
          description: "Emulate a phone screen: WIDTH,HEIGHT,PIXEL_RATIO",
          args: {
            name: "value",
          },
        },
        {
          name: "--proxy-bypass-list",
          description: "Hosts that skip the proxy",
          args: {
            name: "value",
          },
        },
        {
          name: "--driver-version",
          description: "The driver version: a number, latest, stable, beta, ...",
          args: {
            name: "value",
          },
        },
        {
          name: "--cap-file",
          description: "A JSON file of capabilities to add to the session's",
          args: {
            name: "path",
            template: "filepaths",
          },
        },
        {
          name: "--cap-string",
          description: "A JSON object of capabilities to add to the session's",
          args: {
            name: "value",
          },
        },
        {
          name: ["--extension-zip", "--crx"],
          description: "Extension .zip or .crx files to load, separated by commas",
          args: {
            name: "value",
          },
        },
        {
          name: "--firefox-arg",
          description: "Firefox arguments, separated by commas",
          args: {
            name: "value",
          },
        },
        {
          name: "--firefox-pref",
          description: "Firefox preferences, NAME:VALUE separated by commas",
          args: {
            name: "value",
          },
        },
        {
          name: "--xvfb",
          description: "Open a virtual display for a headed browser on Linux",
          args: {
            name: "xvfb",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--xvfb-metrics",
          description: "The virtual display size: WIDTH,HEIGHT",
          args: {
            name: "value",
          },
        },
        {
          name: ["--remote-debug", "--remote-debugger"],
          description: "Open Chrome's remote debugger on port 9222",
          args: {
            name: "remote_debug",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--log-cdp-events", "--log-cdp"],
          description: "Log the DevTools Protocol events",
          args: {
            name: "log_cdp_events",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--uc-cdp-events", "--uc-cdp"],
          description: "Log the DevTools Protocol events in undetected mode",
          args: {
            name: "uc_cdp_events",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--demo", "--demo-mode"],
          description: "Highlight each element and pause after an action",
          args: {
            name: "demo",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--slow", "--slow-mode", "--slowmo"],
          description: "Pause briefly after each action",
          args: {
            name: "slow",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--demo-sleep",
          description: "Seconds to pause after each action in demo mode",
          args: {
            name: "seconds",
          },
        },
        {
          name: "--message-duration",
          description: "Seconds a message stays on the page",
          args: {
            name: "seconds",
          },
        },
        {
          name: ["--skip-js-waits", "--sjw"],
          description: "Do not wait for the page's JavaScript to settle",
          args: {
            name: "skip_js_waits",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--highlights",
          description: "How many times to flash a highlighted element",
          args: {
            name: "count",
          },
        },
        {
          name: "--interval",
          description: "Seconds between the steps of a tour or presentation",
          args: {
            name: "seconds",
          },
        },
        {
          name: "--verify-delay",
          description: "Seconds to wait before each MasterQA verification",
          args: {
            name: "seconds",
          },
        },
        {
          name: "--check-js",
          description: "Check each page for JavaScript errors after it opens",
          args: {
            name: "check_js",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--wait-for-angularjs", "--wfa"],
          description: "Wait for AngularJS after opening a page and clicking",
          args: {
            name: "wait_for_angularjs",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--disable-beforeunload",
          description: "Remove the beforeunload prompt after each page opens",
          args: {
            name: "disable_beforeunload",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--visual-baseline",
          description: "Set the visual baselines again instead of comparing",
          args: {
            name: "visual_baseline",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--crumbs",
          description: "Delete the cookies when a reused browser is handed to the next test",
          args: {
            name: "crumbs",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--timeout-multiplier",
          description: "Multiply the default waits by this number",
          args: {
            name: "value",
          },
        },
        {
          name: ["--rec-sleep", "--record-sleep"],
          description: "Keep the sleeps of a recording",
          args: {
            name: "rec_sleep",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--env",
          description: "The environment the tests run against: qa, staging, production, ...",
          args: {
            name: "env",
            suggestions: ["qa", "rc", "staging", "develop", "production", "performance", "replica", "fedramp", "offline", "online", "master", "remote", "legacy", "local", "alpha", "beta", "demo", "gdpr", "main", "test", "gov", "new", "old", "uat"],
          },
        },
        {
          name: "--account",
          description: "A free account name for the tests",
          args: {
            name: "value",
          },
        },
        {
          name: "--data",
          description: "Free test data",
          args: {
            name: "value",
          },
        },
        {
          name: "--var1",
          description: "A free variable for the tests",
          args: {
            name: "value",
          },
        },
        {
          name: "--var2",
          description: "A free variable for the tests",
          args: {
            name: "value",
          },
        },
        {
          name: "--var3",
          description: "A free variable for the tests",
          args: {
            name: "value",
          },
        },
        {
          name: "--variables",
          description: "A dictionary of values: JSON or a Python dictionary literal",
          args: {
            name: "value",
          },
        },
        {
          name: "--pdb",
          description: "Stop in a post-mortem prompt when a test fails",
          args: {
            name: "pdb",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--trace",
          description: "Stop in a prompt before each test's body",
          args: {
            name: "trace",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--final-debug", "--fdebug", "--final-trace", "--ftrace"],
          description: "Stop in a prompt after each test, before the browser quits",
          args: {
            name: "final_debug",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--reruns",
          description: "Run a failing test again this many times",
          args: {
            name: "count",
          },
        },
        {
          name: "--reruns-delay",
          description: "Seconds to wait between reruns",
          args: {
            name: "seconds",
          },
        },
        {
          name: ["--time-limit", "--timelimit"],
          description: "Fail a test that runs longer than this many seconds",
          args: {
            name: "seconds",
          },
        },
        {
          name: "--runner",
          description: "The test runner: cargo or nextest",
          args: {
            name: "runner",
            suggestions: ["cargo", "nextest"],
          },
        },
        {
          name: ["--screenshot", "--save-screenshot", "--ss"],
          description: "Save a screenshot of passing tests too",
          args: {
            name: "screenshot",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--no-screenshot", "--ns"],
          description: "Save no screenshot of failing tests",
          args: {
            name: "no_screenshot",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--list-fail-page", "--list-fail-pages", "--lfp"],
          description: "Print the URL of the page a failing test ended on",
          args: {
            name: "list_fail_page",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--archive-logs",
          description: "Keep the old latest_logs folder in archived_logs",
          args: {
            name: "archive_logs",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--archive-downloads",
          description: "Keep the old downloaded_files folder in archived_files",
          args: {
            name: "archive_downloads",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--dashboard",
          description: "Write the results dashboard",
          args: {
            name: "dashboard",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--dash-title",
          description: "The dashboard's title",
          args: {
            name: "value",
          },
        },
        {
          name: "--with-db-reporting",
          description: "Record each test's outcome in the results database",
          args: {
            name: "with_db_reporting",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--database-env",
          description: "Which results database to use",
          args: {
            name: "value",
          },
        },
        {
          name: "--with-s3-logging",
          description: "Upload each test's logs to S3",
          args: {
            name: "with_s3_logging",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--rec", "--record", "--recorder", "--codegen"],
          description: "Record the browser actions as a test",
          args: {
            name: "rec",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["--rec-behave", "--rec-gherkin"],
          description: "Also write the recording as a Gherkin feature",
          args: {
            name: "rec_behave",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--rec-print",
          description: "Print the recording when the test ends",
          args: {
            name: "rec_print",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: "--rec-sb-mgr",
          description: "Write the recording as a script",
          args: {
            name: "rec_sb_mgr",
            isOptional: true,
            suggestions: ["true", "false"],
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "filter",
          description: "Run only the tests whose name contains this text.",
          isOptional: true,
        },
        {
          name: "harness_args",
          description: "Arguments for the test harness, after `--` (for example --exact).",
          isOptional: true,
          isVariadic: true,
        },
      ],
    },
    {
      name: "behave-options",
      description: "Print how Python SeleniumBase's behave options map to `sbrs test`.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "options",
      description: "List the browser options: each flag, its `SB_*` variable and its key in the settings file.",
      options: [
        {
          name: "--markdown",
          description: "Print a Markdown table, for the documentation.",
        },
        {
          name: "--all",
          description: "Also list the options of the test run (`sbrs test`).",
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "methods",
      description: "List the common methods of the test API, as they are called on `sb`.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: ["grid-hub", "grid_hub"],
      description: "Start, stop, restart or check a Selenium Grid hub.",
      options: [
        {
          name: "--jar",
          description: "The Selenium Server jar (default: SB_SELENIUM_SERVER_JAR, else the newest in downloaded_drivers/). This command never downloads it.",
          args: {
            name: "path",
            template: "filepaths",
          },
        },
        {
          name: "--java",
          description: "The Java executable (default: `java` on PATH).",
          args: {
            name: "path",
            template: "filepaths",
          },
        },
        {
          name: "--grid-host",
          description: "The address to listen on.",
          args: {
            name: "host",
          },
        },
        {
          name: "--grid-port",
          description: "The port to listen on (default: 4444 for a hub, 5555 for a node).",
          args: {
            name: "port",
          },
        },
        {
          name: "--hub",
          description: "For a node, the hub to register with: a host or a URL.",
          args: {
            name: "host_or_url",
          },
        },
        {
          name: "--timeout",
          description: "Close sessions that have been idle this many seconds.",
          args: {
            name: "seconds",
          },
        },
        {
          name: "--wait",
          description: "How many seconds `start` waits for the Grid to become ready.",
          args: {
            name: "seconds",
          },
        },
        {
          name: ["-v", "--verbose"],
          description: "Log at INFO instead of WARNING.",
        },
        {
          name: "--skip-port-check",
          description: "Start even if something already accepts connections on the port.",
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "action",
          description: "What to do with the Grid process.",
          suggestions: ["start", "stop", "restart", "status"],
        },
        {
          name: "extra_args",
          description: "More arguments for the Selenium Server, after `--`.",
          isOptional: true,
          isVariadic: true,
        },
      ],
    },
    {
      name: ["grid-node", "grid_node"],
      description: "Start, stop, restart or check a Selenium Grid node.",
      options: [
        {
          name: "--jar",
          description: "The Selenium Server jar (default: SB_SELENIUM_SERVER_JAR, else the newest in downloaded_drivers/). This command never downloads it.",
          args: {
            name: "path",
            template: "filepaths",
          },
        },
        {
          name: "--java",
          description: "The Java executable (default: `java` on PATH).",
          args: {
            name: "path",
            template: "filepaths",
          },
        },
        {
          name: "--grid-host",
          description: "The address to listen on.",
          args: {
            name: "host",
          },
        },
        {
          name: "--grid-port",
          description: "The port to listen on (default: 4444 for a hub, 5555 for a node).",
          args: {
            name: "port",
          },
        },
        {
          name: "--hub",
          description: "For a node, the hub to register with: a host or a URL.",
          args: {
            name: "host_or_url",
          },
        },
        {
          name: "--timeout",
          description: "Close sessions that have been idle this many seconds.",
          args: {
            name: "seconds",
          },
        },
        {
          name: "--wait",
          description: "How many seconds `start` waits for the Grid to become ready.",
          args: {
            name: "seconds",
          },
        },
        {
          name: ["-v", "--verbose"],
          description: "Log at INFO instead of WARNING.",
        },
        {
          name: "--skip-port-check",
          description: "Start even if something already accepts connections on the port.",
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "action",
          description: "What to do with the Grid process.",
          suggestions: ["start", "stop", "restart", "status"],
        },
        {
          name: "extra_args",
          description: "More arguments for the Selenium Server, after `--`.",
          isOptional: true,
          isVariadic: true,
        },
      ],
    },
    {
      name: "translate",
      description: "Write the actions of a JSON scenario in another language.",
      options: [
        {
          name: ["-p", "--print"],
          description: "Print the translation and write nothing (the default).",
        },
        {
          name: ["-o", "--overwrite"],
          description: "Replace the file with the translation.",
        },
        {
          name: ["-c", "--copy"],
          description: "Write the translation to a new file next to it, such as `login_es.json`.",
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "file",
          description: "The scenario file.",
          template: "filepaths",
        },
        {
          name: "language",
          description: "The language: a code (en, zh, nl, fr, it, ja, ko, pt, ru, es) or its English name.",
          suggestions: ["en", "zh", "nl", "fr", "it", "ja", "ko", "pt", "ru", "es"],
        },
      ],
    },
    {
      name: "mkdir",
      description: "Create a folder with a runnable browser-test suite.",
      options: [
        {
          name: ["-b", "--basic"],
          description: "Create only the scaffolding, with no example tests.",
        },
        {
          name: "--force",
          description: "Replace generated files that already exist.",
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "dir",
          description: "Folder to create, for example `tests/ui`.",
          template: "folders",
        },
      ],
    },
    {
      name: ["mkfile", "new"],
      description: "Create a browser-test file.",
      options: [
        {
          name: "--url",
          description: "Page the test opens (default: https://example.com).",
          args: {
            name: "url",
          },
        },
        {
          name: ["-b", "--basic"],
          description: "Leave the test body empty: it only opens the page.",
        },
        {
          name: "--force",
          description: "Replace the file if it already exists.",
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "file",
          description: "File to create, for example `tests/login.rs`.",
          template: "filepaths",
        },
      ],
    },
    {
      name: ["commander", "gui"],
      description: "Launch the Commander, a terminal app (needs a build with the `tui` feature).",
      options: [
        {
          name: "-k",
          description: "Start with the list narrowed to the names that contain FILTER.",
          args: {
            name: "filter",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "path",
          description: "A project folder, a folder of test files, or one test file.",
          isOptional: true,
          template: "filepaths",
        },
      ],
    },
    {
      name: ["caseplans", "case-plans", "case_plans"],
      description: "Launch the Case Plans Generator, a terminal app (needs a build with the `tui` feature).",
      options: [
        {
          name: "-k",
          description: "Start with the list narrowed to the tests whose id contains FILTER.",
          args: {
            name: "filter",
          },
        },
        {
          name: "--starter",
          description: "Write the starter checklist `CASE_PLANS.md` instead; this needs no `tui` feature.",
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "path",
          description: "A folder of tests, or one test file.",
          isOptional: true,
          template: "filepaths",
        },
      ],
    },
    {
      name: ["behave-gui", "gui-behave"],
      description: "Launch the Behave Commander, a terminal app (needs a build with the `tui` feature).",
      options: [
        {
          name: "--new",
          description: "Prompt for a name, a title and scenarios and write a new feature file instead; this needs no `tui` feature.",
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "path",
          description: "A folder of feature files, or one `.feature` file.",
          isOptional: true,
          template: "filepaths",
        },
      ],
    },
    {
      name: "print",
      description: "Print a file.",
      options: [
        {
          name: "--file",
          description: "File path to print",
          isRequired: true,
          args: {
            name: "file",
            template: "filepaths",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "objectify",
      description: "Objectify a test recording.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "mkpres",
      description: "Create an HTML presentation.",
      options: [
        {
          name: "--file",
          description: "Output HTML presentation path",
          isRequired: true,
          args: {
            name: "file",
            template: "filepaths",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "mkchart",
      description: "Create an HTML chart.",
      options: [
        {
          name: "--file",
          description: "Output HTML chart path",
          isRequired: true,
          args: {
            name: "file",
            template: "filepaths",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: ["mkrec", "codegen", "record"],
      description: "Create a new test file with the Recorder: record your actions in a browser, then save them as a test.",
      options: [
        {
          name: "--overwrite",
          description: "Overwrite the file when it exists",
        },
        {
          name: ["--behave", "--gherkin"],
          description: "Also write the recording as a Gherkin feature (same as --rec-behave)",
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "file",
          description: "The test file to create (FILE.rs)",
          template: "filepaths",
        },
      ],
    },
    {
      name: "run-scenario",
      description: "Run a scenario file, the way to do several steps in one browser, and write an optional dashboard.",
      options: [
        {
          name: "--file",
          description: "Scenario file to run",
          isRequired: true,
          args: {
            name: "file",
            template: "filepaths",
          },
        },
        {
          name: "--dashboard",
          description: "Optional path to write a scenario dashboard",
          args: {
            name: "dashboard",
            template: "filepaths",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "completions",
      description: "Generate a completion script for a supported shell, or a Fig spec.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "shell",
          description: "Shell (or `fig` for a Fig spec) to generate completions for",
          suggestions: ["bash", "zsh", "fish", "elvish", "powershell", "fig", "usage"],
        },
      ],
    },
    {
      name: "import-python",
      description: "Convert common SeleniumBase or Selenium Python code into a Rust test.",
      options: [
        {
          name: ["-o", "--output"],
          description: "Output file for the generated Rust test",
          args: {
            name: "output",
            template: "filepaths",
          },
        },
        {
          name: "--source",
          description: "Source API to assume when converting Python code",
          args: {
            name: "source",
            suggestions: ["auto", "selenium-base", "selenium"],
          },
        },
        {
          name: "--test-name",
          description: "Name of the generated test function",
          args: {
            name: "test_name",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "file",
          description: "Python source file to convert.",
          template: "filepaths",
        },
      ],
    },
    {
      name: ["import-playwright", "import-pw"],
      description: "Convert Playwright tests or a script (JavaScript, TypeScript or Python) into Rust tests for the pure CDP engine.",
      options: [
        {
          name: ["-o", "--output"],
          description: "Output file for the generated Rust tests",
          args: {
            name: "output",
            template: "filepaths",
          },
        },
        {
          name: "--language",
          description: "Source language (default: from the file extension, else from the content)",
          args: {
            name: "language",
            suggestions: ["js", "ts", "python"],
          },
        },
        {
          name: "--test-name",
          description: "Name of the test generated for a script without test titles (default: the file name)",
          args: {
            name: "test_name",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "file",
          description: "Playwright source file to convert.",
          template: "filepaths",
        },
      ],
    },
    {
      name: ["import-puppeteer", "import-pptr"],
      description: "Convert Puppeteer scripts or tests (JavaScript, TypeScript) or a Chrome Recorder JSON export into Rust tests for the pure CDP engine.",
      options: [
        {
          name: ["-o", "--output"],
          description: "Output file for the generated Rust tests",
          args: {
            name: "output",
            template: "filepaths",
          },
        },
        {
          name: "--language",
          description: "Source language (default: from the file extension, else from the content)",
          args: {
            name: "language",
            suggestions: ["js", "ts", "json"],
          },
        },
        {
          name: "--test-name",
          description: "Name of the test generated for a script without test titles (default: the file name)",
          args: {
            name: "test_name",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "file",
          description: "Puppeteer source file (or Recorder `.json` export) to convert.",
          template: "filepaths",
        },
      ],
    },
    {
      name: "download-file",
      description: "Download a file from a URL (browser download with an HTTP fallback).",
      options: [
        {
          name: ["-o", "--output"],
          description: "Output file path (defaults to the URL file name in the current directory)",
          args: {
            name: "output",
            template: "filepaths",
          },
        },
        {
          name: "--timeout",
          description: "Seconds to wait for the browser download before falling back to HTTP",
          args: {
            name: "timeout",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "url",
          description: "URL of the file to download",
        },
      ],
    },
    {
      name: "print-pdf",
      description: "Print a page to PDF and save it to disk.",
      options: [
        {
          name: ["-o", "--output"],
          description: "Output PDF file path",
          args: {
            name: "output",
            template: "filepaths",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "url",
          description: "URL to render as a PDF",
        },
      ],
    },
    {
      name: "pdf-text",
      description: "Print the text extracted from a page or PDF URL.",
      options: [
        {
          name: "--save-pdf",
          description: "Also keep the intermediate PDF at this path",
          args: {
            name: "save_pdf",
            template: "filepaths",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "url",
          description: "URL of the page or PDF to extract text from",
        },
      ],
    },
    {
      name: "assert-pdf-text",
      description: "Assert that a page or PDF URL contains the expected text.",
      options: [
        {
          name: "--text",
          description: "Text that must appear in the PDF",
          isRequired: true,
          args: {
            name: "text",
          },
        },
        {
          name: "--save-pdf",
          description: "Also keep the intermediate PDF at this path",
          args: {
            name: "save_pdf",
            template: "filepaths",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "url",
          description: "URL of the page or PDF to check",
        },
      ],
    },
    {
      name: "choose-file",
      description: "Upload a local file into a file input.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "selector",
          description: "CSS selector of the <input type=file> element",
        },
        {
          name: "file",
          description: "Local file to upload",
          template: "filepaths",
        },
      ],
    },
    {
      name: "deferred",
      description: "Run several assertions, then report every failure at once.",
      options: [
        {
          name: "--assert",
          description: "Assertion spec: element=<css>, text=<css>:<text>, title=<text>, or url=<text>",
          isRepeatable: true,
          args: {
            name: "spec",
          },
        },
        {
          name: "--spec",
          description: "JSON file containing an array of assertion specs",
          args: {
            name: "spec",
            template: "filepaths",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "replay",
      description: "Replay a recorded action file (recordings/NAME_rec.json) against a fresh browser.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "file",
          description: "JSON or YAML action file (recordings/NAME_rec.json)",
          template: "filepaths",
        },
      ],
    },
    {
      name: "devtools",
      description: "Launch Chrome with an open DevTools endpoint and print MCP client configs.",
      options: [
        {
          name: "--port",
          description: "DevTools port on 127.0.0.1 (0 lets Chrome pick a free one)",
          args: {
            name: "port",
          },
        },
        {
          name: "--browser-path",
          description: "Browser binary to launch, such as Microsoft Edge or another Chromium browser",
          args: {
            name: "path",
          },
        },
        {
          name: "--json",
          description: "Print the endpoint and configs as one line of JSON instead of text",
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "plugin",
      description: "List the built-in plugins, or show what one declares.",
      subcommands: [
        {
          name: "list",
          description: "List the built-in plugins of this build, and whether they are on.",
          options: [
            {
              name: ["-h", "--help"],
              description: "Print help",
            },
          ],
        },
        {
          name: "info",
          description: "Show what one plugin declares and what it may do.",
          options: [
            {
              name: ["-h", "--help"],
              description: "Print help",
            },
          ],
          args: [
            {
              name: "name",
              description: "The plugin's name, as `sbrs plugin list` shows it.",
            },
          ],
        },
      ],
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "fingerprint",
      description: "Capture, write, check and compare browser fingerprints.",
      subcommands: [
        {
          name: "capture",
          description: "Read the fingerprint of a real browser and write it as a fingerprint file.",
          options: [
            {
              name: ["-o", "--output"],
              description: "Write the fingerprint to FILE instead of printing it.",
              args: {
                name: "file",
                template: "filepaths",
              },
            },
            {
              name: "--readings",
              description: "Also write the readings that no setting holds, and where the capture was taken, to FILE.",
              args: {
                name: "file",
                template: "filepaths",
              },
            },
            {
              name: ["-h", "--help"],
              description: "Print help",
            },
          ],
          args: [
            {
              name: "url",
              description: "The page to open first (default: a blank page).",
              isOptional: true,
            },
          ],
        },
        {
          name: "export",
          description: "Write a preset or a random identity as a fingerprint file.",
          options: [
            {
              name: "--preset",
              description: "The preset to write: windows, edge, macos, linux, android or ios (safari is ios).",
              args: {
                name: "name",
                suggestions: ["windows", "edge", "macos", "linux", "android", "ios"],
              },
            },
            {
              name: "--random",
              description: "Write a random identity instead of a preset.",
            },
            {
              name: "--seed",
              description: "With --random, the seed of the draw: the same seed gives the same identity.",
              args: {
                name: "n",
              },
            },
            {
              name: "--os",
              description: "With --random, the system the identity claims (default: this machine's).",
              args: {
                name: "os",
                suggestions: ["windows", "macos", "linux", "android", "ios"],
              },
            },
            {
              name: ["-o", "--output"],
              description: "Write the fingerprint to FILE instead of printing it.",
              args: {
                name: "file",
                template: "filepaths",
              },
            },
            {
              name: ["-h", "--help"],
              description: "Print help",
            },
          ],
        },
        {
          name: "check",
          description: "Check that a fingerprint file loads and is coherent.",
          options: [
            {
              name: ["-h", "--help"],
              description: "Print help",
            },
          ],
          args: [
            {
              name: "file",
              description: "The fingerprint file to check.",
              template: "filepaths",
            },
          ],
        },
        {
          name: "diff",
          description: "Compare two fingerprint files.",
          options: [
            {
              name: ["-h", "--help"],
              description: "Print help",
            },
          ],
          args: [
            {
              name: "expected",
              description: "The file with what is expected.",
              template: "filepaths",
            },
            {
              name: "found",
              description: "The file with what is found.",
              template: "filepaths",
            },
          ],
        },
        {
          name: "presets",
          description: "List the presets and what each claims.",
          options: [
            {
              name: ["-h", "--help"],
              description: "Print help",
            },
          ],
        },
      ],
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "help",
      description: "Print this message or the help of the given subcommand(s)",
      args: { name: "subcommand", isOptional: true, isVariadic: true },
    },
  ],
  options: [
    {
      name: "--webdriver",
      description: "WebDriver server URL (default: SB_WEBDRIVER_URL, else http://localhost:4444)",
      isPersistent: true,
      args: {
        name: "url",
      },
    },
    {
      name: "--browser",
      description: "Browser engine to launch (default: chrome, or the config file's)",
      isPersistent: true,
      args: {
        name: "browser",
        suggestions: ["chrome", "chromium", "edge", "firefox", "brave", "comet", "opera", "safari", "ie", "cft", "chs"],
      },
    },
    {
      name: ["--headed", "--gui"],
      description: "Run the browser in headed mode",
      isPersistent: true,
    },
    {
      name: ["--headless", "--headless2"],
      description: "Run the browser in headless mode",
      isPersistent: true,
    },
    {
      name: "--cdp",
      description: "Enable Chrome DevTools Protocol mode",
      isPersistent: true,
    },
    {
      name: ["--uc", "--undetected", "--undetectable"],
      description: "Enable undetected-chromedriver style evasions",
      isPersistent: true,
    },
    {
      name: "--user-agent",
      description: "Override the default user agent string",
      isPersistent: true,
      args: {
        name: "user_agent",
      },
    },
    {
      name: ["-a", "--agent"],
      description: "Alias for --user-agent",
      isPersistent: true,
      args: {
        name: "agent",
      },
    },
    {
      name: "--locale",
      description: "Locale string for the browser context",
      isPersistent: true,
      args: {
        name: "locale",
      },
    },
    {
      name: ["--start-page", "--url", "--start_page"],
      description: "Page the browser opens first, Python's --start-page (aliases --url and --start_page)",
      isPersistent: true,
      args: {
        name: "url",
      },
    },
    {
      name: ["--save-screenshot", "--save_screenshot"],
      description: "Save a screenshot to latest_logs/ just before the browser closes, and print its path",
      isPersistent: true,
    },
    {
      name: ["--ad-block", "--adblock", "--block-ads"],
      description: "Refuse the requests of ad and tracker hosts (Chromium WebDriver sessions)",
      isPersistent: true,
    },
    {
      name: ["--proxy", "--proxy-server", "--proxy-string"],
      description: "Proxy (host:port or scheme://host:port); with --proxy-list, a name from the list",
      isPersistent: true,
      args: {
        name: "proxy",
      },
    },
    {
      name: "--proxy-list",
      description: "File of proxies to choose from; without --proxy a random one is used",
      isPersistent: true,
      args: {
        name: "file",
        template: "filepaths",
      },
    },
    {
      name: ["--proxy-pac-url", "--pac-url"],
      description: "URL to a proxy auto-config (PAC) file",
      isPersistent: true,
      args: {
        name: "proxy_pac_url",
      },
    },
    {
      name: "--user-data-dir",
      description: "Path to a persistent browser user-data directory",
      isPersistent: true,
      args: {
        name: "user_data_dir",
        template: "folders",
      },
    },
    {
      name: "--extension-dir",
      description: "Path to an unpacked extension directory",
      isPersistent: true,
      args: {
        name: "extension_dir",
        template: "folders",
      },
    },
    {
      name: ["--reuse-session", "--reuse_session", "--rs"],
      description: "Share browsers among the tests of the run (SB_REUSE_SESSION)",
      isPersistent: true,
    },
    {
      name: ["--rcs", "--reuse-class-session", "--reuse_class_session"],
      description: "Share a browser among the tests of one Rust module (SB_REUSE_CLASS_SESSION)",
      isPersistent: true,
    },
    {
      name: ["--mobile", "--mobile-emulator"],
      description: "Use a mobile device emulation profile",
      isPersistent: true,
    },
    {
      name: ["--esc-end", "--esc_end", "--ee"],
      description: "End the current test when ESC is pressed in the browser",
      isPersistent: true,
    },
    {
      name: ["--rec-sb-cdp", "--rec_sb_cdp"],
      description: "Write recordings as pure CDP code (sb_cdp Page and Locator calls)",
      isPersistent: true,
    },
    {
      name: ["-c", "--config", "--settings-file", "--settings"],
      description: "Path to a TOML/JSON settings file",
      isPersistent: true,
      args: {
        name: "config",
        template: "filepaths",
      },
    },
    {
      name: "--fingerprint",
      description: "Make every browser the command starts present the fingerprint in FILE",
      isPersistent: true,
      args: {
        name: "file",
        template: "filepaths",
      },
    },
    {
      name: "--plugin",
      description: "Turn on a built-in plugin in the browsers the command starts (repeatable)",
      isRepeatable: true,
      isPersistent: true,
      args: {
        name: "name",
      },
    },
    {
      name: "--plugin-opt",
      description: "Set one option of a plugin, NAME.KEY=VALUE with a TOML VALUE (repeatable)",
      isRepeatable: true,
      isPersistent: true,
      args: {
        name: "name.key=value",
      },
    },
    {
      name: ["--binary-location", "--bl"],
      description: "The browser binary to run",
      isPersistent: true,
      args: {
        name: "path",
        template: "filepaths",
      },
    },
    {
      name: "--mode",
      description: "The driver mode: webdriver, cdp or uc",
      isPersistent: true,
      args: {
        name: "value",
      },
    },
    {
      name: "--chrome",
      description: "Run Chrome",
      isPersistent: true,
      args: {
        name: "chrome",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--edge",
      description: "Run Edge",
      isPersistent: true,
      args: {
        name: "edge",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--firefox",
      description: "Run Firefox",
      isPersistent: true,
      args: {
        name: "firefox",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--brave",
      description: "Run Brave",
      isPersistent: true,
      args: {
        name: "brave",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--comet",
      description: "Run Comet",
      isPersistent: true,
      args: {
        name: "comet",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--opera",
      description: "Run Opera",
      isPersistent: true,
      args: {
        name: "opera",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--safari",
      description: "Run Safari",
      isPersistent: true,
      args: {
        name: "safari",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--ie",
      description: "Run Internet Explorer",
      isPersistent: true,
      args: {
        name: "ie",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--cft",
      description: "Run Chrome for Testing",
      isPersistent: true,
      args: {
        name: "cft",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--chs",
      description: "Run chrome-headless-shell",
      isPersistent: true,
      args: {
        name: "chs",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: ["--chromium", "--use-chromium"],
      description: "Run Chromium",
      isPersistent: true,
      args: {
        name: "chromium",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--headless1",
      description: "Run headless with chrome-headless-shell (Chrome's old headless mode)",
      isPersistent: true,
      args: {
        name: "headless1",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--locale-code",
      description: "The browser language code, such as fr or ar_eg",
      isPersistent: true,
      args: {
        name: "value",
      },
    },
    {
      name: "--server",
      description: "The Selenium Grid server to connect to",
      isPersistent: true,
      args: {
        name: "value",
      },
    },
    {
      name: "--port",
      description: "The port of the Selenium Grid server",
      isPersistent: true,
      args: {
        name: "value",
      },
    },
    {
      name: "--protocol",
      description: "The protocol of the Selenium Grid server: http or https",
      isPersistent: true,
      args: {
        name: "value",
      },
    },
    {
      name: "--chromium-arg",
      description: "Extra Chromium switches, separated by commas",
      isPersistent: true,
      args: {
        name: "value",
      },
    },
    {
      name: ["--dark-mode", "--dark"],
      description: "Force Chrome's dark theme on web pages",
      isPersistent: true,
      args: {
        name: "dark_mode",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: ["--devtools", "--open-devtools"],
      description: "Open the developer tools with each tab (headed only)",
      isPersistent: true,
      args: {
        name: "devtools",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--disable-features",
      description: "Chromium features to switch off, separated by commas",
      isPersistent: true,
      args: {
        name: "value",
      },
    },
    {
      name: "--disable-gpu",
      description: "Switch the GPU off",
      isPersistent: true,
      args: {
        name: "disable_gpu",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: ["--disable-ws", "--dws", "--disable-web-security"],
      description: "Switch web security off",
      isPersistent: true,
      args: {
        name: "disable_ws",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: ["--guest-mode", "--guest"],
      description: "Open the browser as a guest",
      isPersistent: true,
      args: {
        name: "guest_mode",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: ["--incognito", "--incognito-mode"],
      description: "Open the browser in an incognito window",
      isPersistent: true,
      args: {
        name: "incognito",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--host-resolver-rules",
      description: "Chromium rules that map host names to addresses",
      isPersistent: true,
      args: {
        name: "value",
      },
    },
    {
      name: "--no-sandbox",
      description: "Start Chromium without its sandbox",
      isPersistent: true,
      args: {
        name: "no_sandbox",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--swiftshader",
      description: "Render with SwiftShader, a software WebGL renderer",
      isPersistent: true,
      args: {
        name: "swiftshader",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--enable-3d-apis",
      description: "Keep the GPU on in a headless session, for WebGL",
      isPersistent: true,
      args: {
        name: "enable_3d_apis",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: ["--maximize", "--maximize-window", "--fullscreen"],
      description: "Maximize the browser window",
      isPersistent: true,
      args: {
        name: "maximize",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--block-images",
      description: "Do not load images",
      isPersistent: true,
      args: {
        name: "block_images",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--disable-js",
      description: "Turn JavaScript off",
      isPersistent: true,
      args: {
        name: "disable_js",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--disable-cookies",
      description: "Refuse every cookie",
      isPersistent: true,
      args: {
        name: "disable_cookies",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--do-not-track",
      description: "Send the DNT: 1 header",
      isPersistent: true,
      args: {
        name: "do_not_track",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--external-pdf",
      description: "Download PDF files instead of showing them",
      isPersistent: true,
      args: {
        name: "external_pdf",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--enable-sync",
      description: "Leave Chrome Sync on",
      isPersistent: true,
      args: {
        name: "enable_sync",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: ["--disable-csp", "--dcsp", "--no-csp"],
      description: "Strip the Content Security Policy from every response",
      isPersistent: true,
      args: {
        name: "disable_csp",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: ["--use-auto-ext", "--auto-ext"],
      description: "Keep Chrome's automation extension",
      isPersistent: true,
      args: {
        name: "use_auto_ext",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--proxy-driver",
      description: "Download the driver through the proxy",
      isPersistent: true,
      args: {
        name: "proxy_driver",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: ["--page-load-strategy", "--pls"],
      description: "How long a navigation waits: normal, eager or none",
      isPersistent: true,
      args: {
        name: "strategy",
        suggestions: ["normal", "eager", "none"],
      },
    },
    {
      name: "--window-size",
      description: "The window size: WIDTH,HEIGHT",
      isPersistent: true,
      args: {
        name: "value",
      },
    },
    {
      name: "--window-position",
      description: "The window position: X,Y",
      isPersistent: true,
      args: {
        name: "value",
      },
    },
    {
      name: ["--device-metrics", "--metrics"],
      description: "Emulate a phone screen: WIDTH,HEIGHT,PIXEL_RATIO",
      isPersistent: true,
      args: {
        name: "value",
      },
    },
    {
      name: "--proxy-bypass-list",
      description: "Hosts that skip the proxy",
      isPersistent: true,
      args: {
        name: "value",
      },
    },
    {
      name: "--driver-version",
      description: "The driver version: a number, latest, stable, beta, ...",
      isPersistent: true,
      args: {
        name: "value",
      },
    },
    {
      name: "--cap-file",
      description: "A JSON file of capabilities to add to the session's",
      isPersistent: true,
      args: {
        name: "path",
        template: "filepaths",
      },
    },
    {
      name: "--cap-string",
      description: "A JSON object of capabilities to add to the session's",
      isPersistent: true,
      args: {
        name: "value",
      },
    },
    {
      name: ["--extension-zip", "--crx"],
      description: "Extension .zip or .crx files to load, separated by commas",
      isPersistent: true,
      args: {
        name: "value",
      },
    },
    {
      name: "--firefox-arg",
      description: "Firefox arguments, separated by commas",
      isPersistent: true,
      args: {
        name: "value",
      },
    },
    {
      name: "--firefox-pref",
      description: "Firefox preferences, NAME:VALUE separated by commas",
      isPersistent: true,
      args: {
        name: "value",
      },
    },
    {
      name: "--xvfb",
      description: "Open a virtual display for a headed browser on Linux",
      isPersistent: true,
      args: {
        name: "xvfb",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--xvfb-metrics",
      description: "The virtual display size: WIDTH,HEIGHT",
      isPersistent: true,
      args: {
        name: "value",
      },
    },
    {
      name: ["--remote-debug", "--remote-debugger"],
      description: "Open Chrome's remote debugger on port 9222",
      isPersistent: true,
      args: {
        name: "remote_debug",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: ["--log-cdp-events", "--log-cdp"],
      description: "Log the DevTools Protocol events",
      isPersistent: true,
      args: {
        name: "log_cdp_events",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: ["--uc-cdp-events", "--uc-cdp"],
      description: "Log the DevTools Protocol events in undetected mode",
      isPersistent: true,
      args: {
        name: "uc_cdp_events",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: ["--demo", "--demo-mode"],
      description: "Highlight each element and pause after an action",
      isPersistent: true,
      args: {
        name: "demo",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: ["--slow", "--slow-mode", "--slowmo"],
      description: "Pause briefly after each action",
      isPersistent: true,
      args: {
        name: "slow",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--demo-sleep",
      description: "Seconds to pause after each action in demo mode",
      isPersistent: true,
      args: {
        name: "seconds",
      },
    },
    {
      name: "--message-duration",
      description: "Seconds a message stays on the page",
      isPersistent: true,
      args: {
        name: "seconds",
      },
    },
    {
      name: ["--skip-js-waits", "--sjw"],
      description: "Do not wait for the page's JavaScript to settle",
      isPersistent: true,
      args: {
        name: "skip_js_waits",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--highlights",
      description: "How many times to flash a highlighted element",
      isPersistent: true,
      args: {
        name: "count",
      },
    },
    {
      name: "--interval",
      description: "Seconds between the steps of a tour or presentation",
      isPersistent: true,
      args: {
        name: "seconds",
      },
    },
    {
      name: "--verify-delay",
      description: "Seconds to wait before each MasterQA verification",
      isPersistent: true,
      args: {
        name: "seconds",
      },
    },
    {
      name: "--check-js",
      description: "Check each page for JavaScript errors after it opens",
      isPersistent: true,
      args: {
        name: "check_js",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: ["--wait-for-angularjs", "--wfa"],
      description: "Wait for AngularJS after opening a page and clicking",
      isPersistent: true,
      args: {
        name: "wait_for_angularjs",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--disable-beforeunload",
      description: "Remove the beforeunload prompt after each page opens",
      isPersistent: true,
      args: {
        name: "disable_beforeunload",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--visual-baseline",
      description: "Set the visual baselines again instead of comparing",
      isPersistent: true,
      args: {
        name: "visual_baseline",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--crumbs",
      description: "Delete the cookies when a reused browser is handed to the next test",
      isPersistent: true,
      args: {
        name: "crumbs",
        isOptional: true,
        suggestions: ["true", "false"],
      },
    },
    {
      name: "--timeout-multiplier",
      description: "Multiply the default waits by this number",
      isPersistent: true,
      args: {
        name: "value",
      },
    },
    {
      name: ["--rec", "--recorder", "--record", "--codegen"],
      description: "Record the browser actions as a test",
      isPersistent: true,
    },
    {
      name: ["--rec-print", "--rec_print"],
      description: "Print the recording when the test ends",
      isPersistent: true,
    },
    {
      name: ["--rec-behave", "--rec_behave", "--rec-gherkin"],
      description: "Also write the recording as a Gherkin feature",
      isPersistent: true,
    },
    {
      name: ["--rec-sb-mgr", "--rec_sb_mgr"],
      description: "Write the recording as a script, not a test",
      isPersistent: true,
    },
    {
      name: ["--rec-sleep", "--record-sleep", "--rec_sleep", "--record_sleep"],
      description: "Keep the sleeps of the recording",
      isPersistent: true,
    },
    {
      name: ["-h", "--help"],
      description: "Print help",
    },
    {
      name: ["-V", "--version"],
      description: "Print version",
    },
  ],
};

export default completionSpec;
