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
      name: "throttle3g",
      description: "Throttle the browser connection to 3G speeds.",
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
      name: "go-back",
      description: "Navigate back in browser history.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "go-forward",
      description: "Navigate forward in browser history.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "refresh",
      description: "Reload the current page.",
      options: [
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
      description: "Accept the active browser alert.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "dismiss-alert",
      description: "Dismiss the active browser alert.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "get-alert-text",
      description: "Print the text of the active alert.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "type-alert-text",
      description: "Type text into the active alert prompt.",
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
      name: "switch-to-window",
      description: "Switch focus to a window by handle.",
      options: [
        {
          name: "--handle",
          description: "Window handle to switch to",
          isRequired: true,
          args: {
            name: "handle",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "switch-to-frame",
      description: "Switch focus to an iframe.",
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
      name: "switch-to-default-content",
      description: "Switch focus back to the main document.",
      options: [
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
      name: "export-recording",
      description: "Turn an action file written by `record` into a Rust test.",
      options: [
        {
          name: ["-o", "--output"],
          description: "Rust test to write (default: the action file's name with .rs)",
          args: {
            name: "output",
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
          description: "Action file written by `sbrs record` (.json or .yaml)",
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
          },
        },
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
      name: "set-window-position",
      description: "Set the browser window position.",
      options: [
        {
          name: "--x",
          description: "Window X coordinate in pixels",
          isRequired: true,
          args: {
            name: "x",
          },
        },
        {
          name: "--y",
          description: "Window Y coordinate in pixels",
          isRequired: true,
          args: {
            name: "y",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "close-window",
      description: "Close the current browser window.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "switch-to-parent-frame",
      description: "Switch focus to the parent frame.",
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
      name: "open-new-window",
      description: "Open a new browser window.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "open-new-tab",
      description: "Open a new browser tab.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "switch-to-newest-window",
      description: "Switch focus to the newest browser window.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "switch-to-default-window",
      description: "Switch focus to the default window.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "get-active-element-css",
      description: "Print a CSS selector for the currently focused element.",
      options: [
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
      description: "Wait for an alert, then accept it.",
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
      description: "Wait for an alert, then dismiss it.",
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
      description: "Download a driver into ./downloaded_drivers/ (Python's `sbrs get`).",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "driver",
          description: "The driver: chromedriver, geckodriver (or firefoxdriver), edgedriver (or msedgedriver).",
          isOptional: true,
        },
        {
          name: "version",
          description: "`latest`, a major version (chromedriver, edgedriver), a full build, or a geckodriver version such as 0.36.0.",
          isOptional: true,
        },
      ],
    },
    {
      name: "test",
      description: "Run the project's tests, or one example, through Cargo.",
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
          description: "Arguments for the test harness, after `--` (for example --nocapture).",
          isOptional: true,
          isVariadic: true,
        },
      ],
    },
    {
      name: "grid-hub",
      description: "Start, stop, restart or check a Selenium Grid hub.",
      options: [
        {
          name: "--jar",
          description: "The Selenium Server jar (default: the SB_SELENIUM_SERVER_JAR variable). It is never downloaded.",
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
          name: "--session-timeout",
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
          name: "--grid-verbose",
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
      name: "grid-node",
      description: "Start, stop, restart or check a Selenium Grid node.",
      options: [
        {
          name: "--jar",
          description: "The Selenium Server jar (default: the SB_SELENIUM_SERVER_JAR variable). It is never downloaded.",
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
          name: "--session-timeout",
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
          name: "--grid-verbose",
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
        },
      ],
    },
    {
      name: "commander",
      description: "Launch the interactive commander (needs a build with the `tui` feature).",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: ["caseplans", "case-plans"],
      description: "Launch the Case Plans Generator, a terminal app (needs a build with the `tui` feature).",
      options: [
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
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "mkrec",
      description: "Create a test recording scaffold, or record a test live with --url.",
      options: [
        {
          name: "--url",
          description: "Open this page in a browser and record what you do there into the file",
          args: {
            name: "url",
          },
        },
        {
          name: "--duration",
          description: "With --url: stop recording after this many seconds (default: until Ctrl-C)",
          args: {
            name: "duration",
          },
        },
        {
          name: "--file",
          description: "Output recording scaffold path",
          isRequired: true,
          args: {
            name: "file",
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "run-scenario",
      description: "Run a scenario file and write an optional dashboard.",
      options: [
        {
          name: "--file",
          description: "Scenario file to run",
          isRequired: true,
          args: {
            name: "file",
          },
        },
        {
          name: "--dashboard",
          description: "Optional path to write a scenario dashboard",
          args: {
            name: "dashboard",
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
          name: "--url",
          description: "Page to open first (defaults to a generated blank upload page)",
          args: {
            name: "url",
          },
        },
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
        },
      ],
    },
    {
      name: "deferred",
      description: "Run several assertions, then report every failure at once.",
      options: [
        {
          name: "--url",
          description: "URL to open before running the assertions",
          args: {
            name: "url",
          },
        },
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
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "record",
      description: "Record browser interactions into a replayable action file.",
      options: [
        {
          name: ["-o", "--output"],
          description: "Output action file (.json or .yaml)",
          args: {
            name: "output",
          },
        },
        {
          name: "--duration",
          description: "Stop recording after this many seconds (default: run until Ctrl-C)",
          args: {
            name: "duration",
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
          description: "URL to open and record",
        },
      ],
    },
    {
      name: "replay",
      description: "Replay a recorded action file against a fresh browser session.",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: [
        {
          name: "file",
          description: "JSON action file produced by `sbrs record`",
        },
      ],
    },
    {
      name: "devtools",
      description: "Launch Chrome with an open DevTools endpoint and print MCP client configs.",
      options: [
        {
          name: "--headless",
          description: "Run the browser without a window",
        },
        {
          name: "--port",
          description: "DevTools port on 127.0.0.1 (0 lets Chrome pick a free one)",
          args: {
            name: "port",
          },
        },
        {
          name: "--user-data-dir",
          description: "Profile directory to keep (default: a throwaway one; never Chrome's own default profile)",
          args: {
            name: "dir",
          },
        },
        {
          name: "--proxy",
          description: "Proxy: SERVER:PORT, USER:PASS@SERVER:PORT or scheme://SERVER:PORT",
          args: {
            name: "proxy",
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
          name: "--url",
          description: "Page to open once the browser runs",
          args: {
            name: "url",
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
      args: {
        name: "url",
      },
    },
    {
      name: "--browser",
      description: "Browser engine to launch (default: chrome, or the config file's)",
      args: {
        name: "browser",
        suggestions: ["chrome", "chromium", "edge", "firefox", "brave", "comet", "opera", "safari", "ie", "cft", "chs"],
      },
    },
    {
      name: "--headed",
      description: "Run the browser in headed mode",
    },
    {
      name: "--headless",
      description: "Run the browser in headless mode",
    },
    {
      name: "--cdp",
      description: "Enable Chrome DevTools Protocol mode",
    },
    {
      name: "--uc",
      description: "Enable undetected-chromedriver style evasions",
    },
    {
      name: "--user-agent",
      description: "Override the default user agent string",
      args: {
        name: "user_agent",
      },
    },
    {
      name: ["-a", "--agent"],
      description: "Alias for --user-agent",
      args: {
        name: "agent",
      },
    },
    {
      name: "--locale",
      description: "Locale string for the browser context",
      args: {
        name: "locale",
      },
    },
    {
      name: "--ad-block",
      description: "Refuse the requests of ad and tracker hosts (Chromium WebDriver sessions)",
    },
    {
      name: "--proxy",
      description: "Proxy (host:port or scheme://host:port); with --proxy-list, a name from the list",
      args: {
        name: "proxy",
      },
    },
    {
      name: "--proxy-list",
      description: "File of proxies to choose from; without --proxy a random one is used",
      args: {
        name: "file",
      },
    },
    {
      name: "--proxy-pac-url",
      description: "URL to a proxy auto-config (PAC) file",
      args: {
        name: "proxy_pac_url",
      },
    },
    {
      name: "--user-data-dir",
      description: "Path to a persistent browser user-data directory",
      args: {
        name: "user_data_dir",
      },
    },
    {
      name: "--extension-dir",
      description: "Path to an unpacked extension directory",
      args: {
        name: "extension_dir",
      },
    },
    {
      name: ["--reuse-session", "--reuse_session", "--rs"],
      description: "Share browsers among the tests of the run (SB_REUSE_SESSION)",
    },
    {
      name: ["--rcs", "--reuse-class-session", "--reuse_class_session"],
      description: "Share a browser among the tests of one Rust module (SB_REUSE_CLASS_SESSION)",
    },
    {
      name: "--mobile",
      description: "Use a mobile device emulation profile",
    },
    {
      name: ["--esc-end", "--esc_end", "--ee"],
      description: "End the current test when ESC is pressed in the browser",
    },
    {
      name: ["--rec-sb-cdp", "--rec_sb_cdp"],
      description: "Write recordings as pure CDP code (sb_cdp Page and Locator calls)",
    },
    {
      name: ["-n", "--threads"],
      description: "Number of parallel threads for test runs",
      args: {
        name: "threads",
      },
    },
    {
      name: ["-c", "--config"],
      description: "Path to a TOML/JSON settings file",
      args: {
        name: "config",
      },
    },
    {
      name: "--fingerprint",
      description: "Make every browser the command starts present the fingerprint in FILE",
      args: {
        name: "file",
        template: "filepaths",
      },
    },
    {
      name: "--plugin",
      description: "Turn on a built-in plugin in the browsers the command starts (repeatable)",
      isRepeatable: true,
      args: {
        name: "name",
      },
    },
    {
      name: "--plugin-opt",
      description: "Set one option of a plugin, NAME.KEY=VALUE with a TOML VALUE (repeatable)",
      isRepeatable: true,
      args: {
        name: "name.key=value",
      },
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
