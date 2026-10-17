const completion: Fig.Spec = {
  name: "sbrs",
  description: "SeleniumBaseRust CLI",
  subcommands: [
    {
      name: "version",
      description: "Print the sbrs version, as `sbrs --version` does",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "open",
      description: "Open a URL in the browser",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: {
        name: "url",
      },
    },
    {
      name: "smoke",
      description: "Run a smoke test against a URL",
      options: [
        {
          name: "--title-contains",
          description: "Expected substring in the page title",
          isRepeatable: true,
          args: {
            name: "title_contains",
            isOptional: true,
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: {
        name: "url",
      },
    },
    {
      name: "cdp",
      description: "Execute a Chrome DevTools Protocol command",
      options: [
        {
          name: "--cmd",
          description: "CDP method name (e.g. Runtime.evaluate)",
          isRepeatable: true,
          args: {
            name: "cmd",
          },
        },
        {
          name: "--params",
          description: "JSON object of CDP parameters",
          isRepeatable: true,
          args: {
            name: "params",
            isOptional: true,
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
      description: "Clear the browser cache via CDP",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "throttle3g",
      description: "Throttle the browser connection to 3G speeds",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "screenshot",
      description: "Capture a screenshot of the current page",
      options: [
        {
          name: "--path",
          description:
            "Output image file path (default: a new file in latest_logs/)",
          isRepeatable: true,
          args: {
            name: "path",
            isOptional: true,
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
      description: "Save the current page source to a file",
      options: [
        {
          name: "--path",
          description:
            "Output HTML file path (default: a new file in latest_logs/)",
          isRepeatable: true,
          args: {
            name: "path",
            isOptional: true,
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
      description: "Assert that an element exists",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
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
      description: "Wait until an element contains the expected text",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--text",
          description: "Expected text substring to wait for",
          isRepeatable: true,
          args: {
            name: "text",
          },
        },
        {
          name: "--timeout",
          description: "Maximum wait time in seconds",
          isRepeatable: true,
          args: {
            name: "timeout",
            isOptional: true,
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
      description: "Hover over an element",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
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
      description: "Hover over one element, then click another",
      options: [
        {
          name: "--hover-css",
          description: "CSS selector to hover over",
          isRepeatable: true,
          args: {
            name: "hover_css",
          },
        },
        {
          name: "--click-css",
          description: "CSS selector to click",
          isRepeatable: true,
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
      description: "Select an option from a dropdown",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--text",
          description: "Visible text of the option to select",
          isRepeatable: true,
          args: {
            name: "text",
            isOptional: true,
          },
        },
        {
          name: "--value",
          description: "Option value attribute",
          isRepeatable: true,
          args: {
            name: "value",
            isOptional: true,
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
      description: "Drag one element onto another",
      options: [
        {
          name: "--source-css",
          description: "CSS selector of the element to drag",
          isRepeatable: true,
          args: {
            name: "source_css",
          },
        },
        {
          name: "--target-css",
          description: "CSS selector of the drop target",
          isRepeatable: true,
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
      description: "Click an element using CDP",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
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
      description: "Type text using CDP input injection",
      options: [
        {
          name: "--text",
          description: "Text to inject via CDP",
          isRepeatable: true,
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
      description: "Navigate back in browser history",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "go-forward",
      description: "Navigate forward in browser history",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "refresh",
      description: "Reload the current page",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "get-text",
      description: "Print the visible text of an element",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
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
      description: "Print an element attribute value",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--attribute",
          description: "Attribute name to read or assert",
          isRepeatable: true,
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
      description: "Print an element property value",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--property",
          description: "Property name to read",
          isRepeatable: true,
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
      description: "Print the page title",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "get-current-url",
      description: "Print the current URL",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "clear-cookies",
      description: "Delete all browser cookies",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "accept-alert",
      description: "Accept the active browser alert",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "dismiss-alert",
      description: "Dismiss the active browser alert",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "get-alert-text",
      description: "Print the text of the active alert",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "type-alert-text",
      description: "Type text into the active alert prompt",
      options: [
        {
          name: "--text",
          description: "Text to type into the alert prompt",
          isRepeatable: true,
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
      description: "Clear the page local storage",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "get-local-storage-item",
      description: "Print a local storage value by key",
      options: [
        {
          name: "--key",
          description: "Storage key",
          isRepeatable: true,
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
      description: "Set a local storage key/value pair",
      options: [
        {
          name: "--key",
          description: "Storage key",
          isRepeatable: true,
          args: {
            name: "key",
          },
        },
        {
          name: "--value",
          description: "Value to store in local storage",
          isRepeatable: true,
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
      description: "Remove a local storage entry by key",
      options: [
        {
          name: "--key",
          description: "Storage key",
          isRepeatable: true,
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
      description: "Switch focus to a window by handle",
      options: [
        {
          name: "--handle",
          description: "Window handle to switch to",
          isRepeatable: true,
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
      description: "Switch focus to an iframe",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
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
      description: "Switch focus back to the main document",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "get-cookies",
      description: "Print all browser cookies as JSON",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "export-recording",
      description: "Export the recorded test actions",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "patch-chromedriver",
      description: "Patch a chromedriver binary to reduce detection surface",
      options: [
        {
          name: "--path",
          description: "Path to the chromedriver binary",
          isRepeatable: true,
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
      description: "Patch a Chrome/Chromium binary for native-level spoofing",
      options: [
        {
          name: "--path",
          description: "Path to the Chrome/Chromium binary",
          isRepeatable: true,
          args: {
            name: "path",
          },
        },
        {
          name: "--cache-dir",
          description: "Directory where patched binaries are cached",
          isRepeatable: true,
          args: {
            name: "cache_dir",
            isOptional: true,
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
      description:
        "Encrypt text with a passphrase (AES-256-GCM); read from stdin if TEXT is omitted",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help (see more with '--help')",
        },
      ],
      args: {
        name: "text",
        isOptional: true,
      },
    },
    {
      name: ["decrypt", "unobfuscate"],
      description:
        "Decrypt a token made by `sbrs encrypt`; read from stdin if TOKEN is omitted. Python's `sbrs unobfuscate` is the same command",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: {
        name: "token",
        isOptional: true,
      },
    },
    {
      name: "doctor",
      description:
        "Run a diagnostic check on the environment and configuration",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "assert-text-visible",
      description: "Assert that an element contains the expected text",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--text",
          description: "Expected visible text",
          isRepeatable: true,
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
      description: "Assert that an element does not contain the given text",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--text",
          description: "Text that should not be visible",
          isRepeatable: true,
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
      description: "Assert an element attribute equals an expected value",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--attribute",
          description: "Attribute name to read or assert",
          isRepeatable: true,
          args: {
            name: "attribute",
          },
        },
        {
          name: "--value",
          description: "Expected attribute value",
          isRepeatable: true,
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
      description: "Assert the page title contains the expected text",
      options: [
        {
          name: "--text",
          description: "Expected page title substring",
          isRepeatable: true,
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
      description: "Wait until the document readyState is complete",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "get-window-position",
      description: "Print the browser window position",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "set-window-position",
      description: "Set the browser window position",
      options: [
        {
          name: "--x",
          description: "Window X coordinate in pixels",
          isRepeatable: true,
          args: {
            name: "x",
          },
        },
        {
          name: "--y",
          description: "Window Y coordinate in pixels",
          isRepeatable: true,
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
      description: "Close the current browser window",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "switch-to-parent-frame",
      description: "Switch focus to the parent frame",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "is-element-visible",
      description: "Print whether an element is visible",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
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
      description: "Print whether the expected text is visible in an element",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--text",
          description: "Expected text to check",
          isRepeatable: true,
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
      description: "Wait until an element is no longer visible",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--timeout",
          description: "Maximum wait time in seconds",
          isRepeatable: true,
          args: {
            name: "timeout",
            isOptional: true,
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
      description: "Save browser cookies to a JSON file",
      options: [
        {
          name: "--file",
          description: "JSON file to save cookies to",
          isRepeatable: true,
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
      description: "Load browser cookies from a JSON file",
      options: [
        {
          name: "--file",
          description: "JSON file to load cookies from",
          isRepeatable: true,
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
      description: "Highlight an element, then click it",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
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
      description: "Print whether a checkbox is checked",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
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
      description: "Check a checkbox only if it is unchecked",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
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
      description: "Uncheck a checkbox only if it is checked",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
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
      description: "Open a new browser window",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "open-new-tab",
      description: "Open a new browser tab",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "switch-to-newest-window",
      description: "Switch focus to the newest browser window",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "switch-to-default-window",
      description: "Switch focus to the default window",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "get-active-element-css",
      description: "Print a CSS selector for the currently focused element",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "wait-for-element-present",
      description: "Wait until an element is present in the DOM",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--timeout",
          description: "Maximum wait time in seconds",
          isRepeatable: true,
          args: {
            name: "timeout",
            isOptional: true,
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
      description: "Append text to an input or textarea",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--text",
          description: "Text to append to the element",
          isRepeatable: true,
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
      description: "Send keystrokes to an element",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--text",
          description: "Keys to send to the element",
          isRepeatable: true,
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
      description: "Print the value of a form element",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
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
      description: "Click all visible elements matching a selector",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
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
      description: "Wait for an alert, then accept it",
      options: [
        {
          name: "--timeout",
          description: "Maximum wait time in seconds",
          isRepeatable: true,
          args: {
            name: "timeout",
            isOptional: true,
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
      description: "Wait for an alert, then dismiss it",
      options: [
        {
          name: "--timeout",
          description: "Maximum wait time in seconds",
          isRepeatable: true,
          args: {
            name: "timeout",
            isOptional: true,
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
      description: "Print whether a link with the given text is visible",
      options: [
        {
          name: "--text",
          description: "Exact link text to check",
          isRepeatable: true,
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
      description: "Print whether a link containing the given text is visible",
      options: [
        {
          name: "--text",
          description: "Link text substring to check",
          isRepeatable: true,
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
      description: "Assert a link with the given text is visible",
      options: [
        {
          name: "--text",
          description: "Exact link text to assert",
          isRepeatable: true,
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
      description: "Click a link containing the given text",
      options: [
        {
          name: "--text",
          description: "Link text substring to click",
          isRepeatable: true,
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
      description: "Type text with human-like timing and noise",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--text",
          description: "Text to type with humanized timing",
          isRepeatable: true,
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
      description: "Click an element with human-like timing and noise",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
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
      description: "Smoothly scroll an element into view",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
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
      description: "Perform a UC-mode click on an element",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
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
      description: "Perform UC-mode typing into an element",
      options: [
        {
          name: "--css",
          description: "CSS selector for the target element",
          isRepeatable: true,
          args: {
            name: "css",
          },
        },
        {
          name: "--text",
          description: "Text to type in UC mode",
          isRepeatable: true,
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
      description:
        "Download a driver into ./downloaded_drivers/ (Python's `sbrs get`)",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help (see more with '--help')",
        },
      ],
      args: [
        {
          name: "driver",
          isOptional: true,
        },
        {
          name: "version",
          isOptional: true,
        },
      ],
    },
    {
      name: "test",
      description: "Run the project's tests, or one example, through Cargo",
      options: [
        {
          name: "--test",
          description: "Run one integration test file from tests/",
          exclusiveOn: ["--example"],
          isRepeatable: true,
          args: {
            name: "test",
            isOptional: true,
          },
        },
        {
          name: "--example",
          description: "Run one example from examples/ instead of the tests",
          isRepeatable: true,
          args: {
            name: "example",
            isOptional: true,
          },
        },
        {
          name: "--features",
          description: "Cargo features to enable, separated by commas",
          isRepeatable: true,
          args: {
            name: "features",
            isOptional: true,
          },
        },
        {
          name: "--release",
          description: "Build with the release profile",
        },
        {
          name: ["-h", "--help"],
          description: "Print help (see more with '--help')",
        },
      ],
      args: [
        {
          name: "filter",
          isOptional: true,
        },
        {
          name: "harness_args",
          isVariadic: true,
          isOptional: true,
        },
      ],
    },
    {
      name: "grid-hub",
      description: "Start, stop, restart or check a Selenium Grid hub",
      options: [
        {
          name: "--jar",
          description:
            "The Selenium Server jar (default: the SB_SELENIUM_SERVER_JAR variable). It is never downloaded",
          isRepeatable: true,
          args: {
            name: "jar",
            isOptional: true,
            template: "filepaths",
          },
        },
        {
          name: "--java",
          description: "The Java executable (default: `java` on PATH)",
          isRepeatable: true,
          args: {
            name: "java",
            isOptional: true,
            template: "filepaths",
          },
        },
        {
          name: "--grid-host",
          description: "The address to listen on",
          isRepeatable: true,
          args: {
            name: "host",
            isOptional: true,
          },
        },
        {
          name: "--grid-port",
          description:
            "The port to listen on (default: 4444 for a hub, 5555 for a node)",
          isRepeatable: true,
          args: {
            name: "port",
            isOptional: true,
          },
        },
        {
          name: "--hub",
          description: "For a node, the hub to register with: a host or a URL",
          isRepeatable: true,
          args: {
            name: "hub",
            isOptional: true,
          },
        },
        {
          name: "--session-timeout",
          description: "Close sessions that have been idle this many seconds",
          isRepeatable: true,
          args: {
            name: "session_timeout",
            isOptional: true,
          },
        },
        {
          name: "--wait",
          description:
            "How many seconds `start` waits for the Grid to become ready",
          isRepeatable: true,
          args: {
            name: "wait",
            isOptional: true,
          },
        },
        {
          name: "--grid-verbose",
          description: "Log at INFO instead of WARNING",
        },
        {
          name: "--skip-port-check",
          description:
            "Start even if something already accepts connections on the port",
        },
        {
          name: ["-h", "--help"],
          description: "Print help (see more with '--help')",
        },
      ],
      args: [
        {
          name: "action",
          suggestions: [
            {
              name: "start",
              description: "Start it and wait until it is ready",
            },
            {
              name: "stop",
              description: "Stop it",
            },
            {
              name: "restart",
              description: "Stop it if it is running, then start it",
            },
            {
              name: "status",
              description: "Say whether it is running and ready",
            },
          ],
        },
        {
          name: "extra_args",
          isVariadic: true,
          isOptional: true,
        },
      ],
    },
    {
      name: "grid-node",
      description: "Start, stop, restart or check a Selenium Grid node",
      options: [
        {
          name: "--jar",
          description:
            "The Selenium Server jar (default: the SB_SELENIUM_SERVER_JAR variable). It is never downloaded",
          isRepeatable: true,
          args: {
            name: "jar",
            isOptional: true,
            template: "filepaths",
          },
        },
        {
          name: "--java",
          description: "The Java executable (default: `java` on PATH)",
          isRepeatable: true,
          args: {
            name: "java",
            isOptional: true,
            template: "filepaths",
          },
        },
        {
          name: "--grid-host",
          description: "The address to listen on",
          isRepeatable: true,
          args: {
            name: "host",
            isOptional: true,
          },
        },
        {
          name: "--grid-port",
          description:
            "The port to listen on (default: 4444 for a hub, 5555 for a node)",
          isRepeatable: true,
          args: {
            name: "port",
            isOptional: true,
          },
        },
        {
          name: "--hub",
          description: "For a node, the hub to register with: a host or a URL",
          isRepeatable: true,
          args: {
            name: "hub",
            isOptional: true,
          },
        },
        {
          name: "--session-timeout",
          description: "Close sessions that have been idle this many seconds",
          isRepeatable: true,
          args: {
            name: "session_timeout",
            isOptional: true,
          },
        },
        {
          name: "--wait",
          description:
            "How many seconds `start` waits for the Grid to become ready",
          isRepeatable: true,
          args: {
            name: "wait",
            isOptional: true,
          },
        },
        {
          name: "--grid-verbose",
          description: "Log at INFO instead of WARNING",
        },
        {
          name: "--skip-port-check",
          description:
            "Start even if something already accepts connections on the port",
        },
        {
          name: ["-h", "--help"],
          description: "Print help (see more with '--help')",
        },
      ],
      args: [
        {
          name: "action",
          suggestions: [
            {
              name: "start",
              description: "Start it and wait until it is ready",
            },
            {
              name: "stop",
              description: "Stop it",
            },
            {
              name: "restart",
              description: "Stop it if it is running, then start it",
            },
            {
              name: "status",
              description: "Say whether it is running and ready",
            },
          ],
        },
        {
          name: "extra_args",
          isVariadic: true,
          isOptional: true,
        },
      ],
    },
    {
      name: "translate",
      description: "Write the actions of a JSON scenario in another language",
      options: [
        {
          name: ["-p", "--print"],
          description: "Print the translation and write nothing (the default)",
          exclusiveOn: ["-o", "--overwrite", "-c", "--copy"],
        },
        {
          name: ["-o", "--overwrite"],
          description: "Replace the file with the translation",
          exclusiveOn: ["-c", "--copy"],
        },
        {
          name: ["-c", "--copy"],
          description:
            "Write the translation to a new file next to it, such as `login_es.json`",
        },
        {
          name: ["-h", "--help"],
          description: "Print help (see more with '--help')",
        },
      ],
      args: [
        {
          name: "file",
          template: "filepaths",
        },
        {
          name: "language",
        },
      ],
    },
    {
      name: "mkdir",
      description: "Create a folder with a runnable browser-test suite",
      options: [
        {
          name: ["-b", "--basic"],
          description: "Create only the scaffolding, with no example tests",
        },
        {
          name: "--force",
          description: "Replace generated files that already exist",
        },
        {
          name: ["-h", "--help"],
          description: "Print help (see more with '--help')",
        },
      ],
      args: {
        name: "dir",
      },
    },
    {
      name: ["mkfile", "new"],
      description: "Create a browser-test file",
      options: [
        {
          name: "--url",
          description: "Page the test opens (default: https://example.com)",
          isRepeatable: true,
          args: {
            name: "url",
            isOptional: true,
          },
        },
        {
          name: ["-b", "--basic"],
          description: "Leave the test body empty: it only opens the page",
        },
        {
          name: "--force",
          description: "Replace the file if it already exists",
        },
        {
          name: ["-h", "--help"],
          description: "Print help (see more with '--help')",
        },
      ],
      args: {
        name: "file",
      },
    },
    {
      name: "commander",
      description:
        "Launch the interactive commander (needs a build with the `tui` feature)",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: ["caseplans", "case-plans"],
      description:
        "Launch the Case Plans Generator, a terminal app (needs a build with the `tui` feature)",
      options: [
        {
          name: "--starter",
          description:
            "Write the starter checklist `CASE_PLANS.md` instead; this needs no `tui` feature",
        },
        {
          name: ["-h", "--help"],
          description: "Print help (see more with '--help')",
        },
      ],
      args: {
        name: "path",
        isOptional: true,
      },
    },
    {
      name: ["behave-gui", "gui-behave"],
      description:
        "Launch the Behave Commander, a terminal app (needs a build with the `tui` feature)",
      options: [
        {
          name: "--new",
          description:
            "Prompt for a name, a title and scenarios and write a new feature file instead; this needs no `tui` feature",
        },
        {
          name: ["-h", "--help"],
          description: "Print help (see more with '--help')",
        },
      ],
      args: {
        name: "path",
        isOptional: true,
      },
    },
    {
      name: "print",
      description: "Print a file",
      options: [
        {
          name: "--file",
          description: "File path to print",
          isRepeatable: true,
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
      description: "Objectify a test recording",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
    },
    {
      name: "mkpres",
      description: "Create an HTML presentation",
      options: [
        {
          name: "--file",
          description: "Output HTML presentation path",
          isRepeatable: true,
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
      description: "Create an HTML chart",
      options: [
        {
          name: "--file",
          description: "Output HTML chart path",
          isRepeatable: true,
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
      description: "Create a test recording scaffold",
      options: [
        {
          name: "--file",
          description: "Output recording scaffold path",
          isRepeatable: true,
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
      description: "Run a scenario file and write an optional dashboard",
      options: [
        {
          name: "--file",
          description: "Scenario file to run",
          isRepeatable: true,
          args: {
            name: "file",
          },
        },
        {
          name: "--dashboard",
          description: "Optional path to write a scenario dashboard",
          isRepeatable: true,
          args: {
            name: "dashboard",
            isOptional: true,
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
      description:
        "Generate a completion script for a supported shell, or a Fig spec",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help (see more with '--help')",
        },
      ],
      args: {
        name: "shell",
        suggestions: [
          "bash",
          "zsh",
          "fish",
          "elvish",
          "powershell",
          {
            name: "fig",
            description:
              "A Fig completion spec (TypeScript), the format of the withfig/autocomplete repository that Warp's built-in completions, inshellisense and Amazon Q use",
          },
          {
            name: "usage",
            description:
              "A usage spec (KDL) for jdx/usage and mise, which turn it into completions, Markdown and man pages",
          },
        ],
      },
    },
    {
      name: "import-python",
      description:
        "Convert common SeleniumBase or Selenium Python code into a Rust test",
      options: [
        {
          name: ["-o", "--output"],
          description: "Output file for the generated Rust test",
          isRepeatable: true,
          args: {
            name: "output",
            isOptional: true,
          },
        },
        {
          name: "--source",
          description: "Source API to assume when converting Python code",
          isRepeatable: true,
          args: {
            name: "source",
            isOptional: true,
            suggestions: ["auto", "selenium-base", "selenium"],
          },
        },
        {
          name: "--test-name",
          description: "Name of the generated test function",
          isRepeatable: true,
          args: {
            name: "test_name",
            isOptional: true,
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: {
        name: "file",
      },
    },
    {
      name: ["import-playwright", "import-pw"],
      description:
        "Convert Playwright tests or a script (JavaScript, TypeScript or Python) into Rust tests for the pure CDP engine",
      options: [
        {
          name: ["-o", "--output"],
          description: "Output file for the generated Rust tests",
          isRepeatable: true,
          args: {
            name: "output",
            isOptional: true,
          },
        },
        {
          name: "--language",
          description:
            "Source language (default: from the file extension, else from the content)",
          isRepeatable: true,
          args: {
            name: "language",
            isOptional: true,
            suggestions: [
              {
                name: "js",
                description: "JavaScript",
              },
              {
                name: "ts",
                description: "TypeScript",
              },
              {
                name: "python",
                description: "Python",
              },
            ],
          },
        },
        {
          name: "--test-name",
          description:
            "Name of the test generated for a script without test titles (default: the file name)",
          isRepeatable: true,
          args: {
            name: "test_name",
            isOptional: true,
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help (see more with '--help')",
        },
      ],
      args: {
        name: "file",
      },
    },
    {
      name: ["import-puppeteer", "import-pptr"],
      description:
        "Convert Puppeteer scripts or tests (JavaScript, TypeScript) or a Chrome Recorder JSON export into Rust tests for the pure CDP engine",
      options: [
        {
          name: ["-o", "--output"],
          description: "Output file for the generated Rust tests",
          isRepeatable: true,
          args: {
            name: "output",
            isOptional: true,
          },
        },
        {
          name: "--language",
          description:
            "Source language (default: from the file extension, else from the content)",
          isRepeatable: true,
          args: {
            name: "language",
            isOptional: true,
            suggestions: [
              {
                name: "js",
                description: "JavaScript",
              },
              {
                name: "ts",
                description: "TypeScript",
              },
              {
                name: "json",
                description: "A Chrome DevTools Recorder JSON export",
              },
            ],
          },
        },
        {
          name: "--test-name",
          description:
            "Name of the test generated for a script without test titles (default: the file name)",
          isRepeatable: true,
          args: {
            name: "test_name",
            isOptional: true,
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help (see more with '--help')",
        },
      ],
      args: {
        name: "file",
      },
    },
    {
      name: "download-file",
      description:
        "Download a file from a URL (browser download with an HTTP fallback)",
      options: [
        {
          name: ["-o", "--output"],
          description:
            "Output file path (defaults to the URL file name in the current directory)",
          isRepeatable: true,
          args: {
            name: "output",
            isOptional: true,
          },
        },
        {
          name: "--timeout",
          description:
            "Seconds to wait for the browser download before falling back to HTTP",
          isRepeatable: true,
          args: {
            name: "timeout",
            isOptional: true,
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: {
        name: "url",
      },
    },
    {
      name: "print-pdf",
      description: "Print a page to PDF and save it to disk",
      options: [
        {
          name: ["-o", "--output"],
          description: "Output PDF file path",
          isRepeatable: true,
          args: {
            name: "output",
            isOptional: true,
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: {
        name: "url",
      },
    },
    {
      name: "pdf-text",
      description: "Print the text extracted from a page or PDF URL",
      options: [
        {
          name: "--save-pdf",
          description: "Also keep the intermediate PDF at this path",
          isRepeatable: true,
          args: {
            name: "save_pdf",
            isOptional: true,
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: {
        name: "url",
      },
    },
    {
      name: "assert-pdf-text",
      description: "Assert that a page or PDF URL contains the expected text",
      options: [
        {
          name: "--text",
          description: "Text that must appear in the PDF",
          isRepeatable: true,
          args: {
            name: "text",
          },
        },
        {
          name: "--save-pdf",
          description: "Also keep the intermediate PDF at this path",
          isRepeatable: true,
          args: {
            name: "save_pdf",
            isOptional: true,
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: {
        name: "url",
      },
    },
    {
      name: "choose-file",
      description: "Upload a local file into a file input",
      options: [
        {
          name: "--url",
          description:
            "Page to open first (defaults to a generated blank upload page)",
          isRepeatable: true,
          args: {
            name: "url",
            isOptional: true,
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
        },
        {
          name: "file",
        },
      ],
    },
    {
      name: "deferred",
      description: "Run several assertions, then report every failure at once",
      options: [
        {
          name: "--url",
          description: "URL to open before running the assertions",
          isRepeatable: true,
          args: {
            name: "url",
            isOptional: true,
          },
        },
        {
          name: "--assert",
          description:
            "Assertion spec: element=<css>, text=<css>:<text>, title=<text>, or url=<text>",
          isRepeatable: true,
          args: {
            name: "asserts",
            isOptional: true,
          },
        },
        {
          name: "--spec",
          description: "JSON file containing an array of assertion specs",
          isRepeatable: true,
          args: {
            name: "spec",
            isOptional: true,
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
      description: "Record browser interactions into a replayable action file",
      options: [
        {
          name: ["-o", "--output"],
          description: "Output action file (.json or .yaml)",
          isRepeatable: true,
          args: {
            name: "output",
            isOptional: true,
          },
        },
        {
          name: "--duration",
          description:
            "Stop recording after this many seconds (default: run until Ctrl-C)",
          isRepeatable: true,
          args: {
            name: "duration",
            isOptional: true,
          },
        },
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: {
        name: "url",
      },
    },
    {
      name: "replay",
      description:
        "Replay a recorded action file against a fresh browser session",
      options: [
        {
          name: ["-h", "--help"],
          description: "Print help",
        },
      ],
      args: {
        name: "file",
      },
    },
    {
      name: "devtools",
      description:
        "Launch Chrome with an open DevTools endpoint and print MCP client configs",
      options: [
        {
          name: "--port",
          description:
            "DevTools port on 127.0.0.1 (0 lets Chrome pick a free one)",
          isRepeatable: true,
          args: {
            name: "port",
            isOptional: true,
          },
        },
        {
          name: "--user-data-dir",
          description:
            "Profile directory to keep (default: a throwaway one; never Chrome's own default profile)",
          isRepeatable: true,
          args: {
            name: "user_data_dir",
            isOptional: true,
          },
        },
        {
          name: "--proxy",
          description:
            "Proxy: SERVER:PORT, USER:PASS@SERVER:PORT or scheme://SERVER:PORT",
          isRepeatable: true,
          args: {
            name: "proxy",
            isOptional: true,
          },
        },
        {
          name: "--browser-path",
          description:
            "Browser binary to launch, such as Microsoft Edge or another Chromium browser",
          isRepeatable: true,
          args: {
            name: "browser_path",
            isOptional: true,
          },
        },
        {
          name: "--url",
          description: "Page to open once the browser runs",
          isRepeatable: true,
          args: {
            name: "url",
            isOptional: true,
          },
        },
        {
          name: "--headless",
          description: "Run the browser without a window",
        },
        {
          name: "--json",
          description:
            "Print the endpoint and configs as one line of JSON instead of text",
        },
        {
          name: ["-h", "--help"],
          description: "Print help (see more with '--help')",
        },
      ],
    },
    {
      name: "help",
      description: "Print this message or the help of the given subcommand(s)",
      subcommands: [
        {
          name: "version",
          description: "Print the sbrs version, as `sbrs --version` does",
        },
        {
          name: "open",
          description: "Open a URL in the browser",
        },
        {
          name: "smoke",
          description: "Run a smoke test against a URL",
        },
        {
          name: "cdp",
          description: "Execute a Chrome DevTools Protocol command",
        },
        {
          name: "cache-clear",
          description: "Clear the browser cache via CDP",
        },
        {
          name: "throttle3g",
          description: "Throttle the browser connection to 3G speeds",
        },
        {
          name: "screenshot",
          description: "Capture a screenshot of the current page",
        },
        {
          name: "save-source",
          description: "Save the current page source to a file",
        },
        {
          name: "assert-element",
          description: "Assert that an element exists",
        },
        {
          name: "wait-for-text",
          description: "Wait until an element contains the expected text",
        },
        {
          name: "hover",
          description: "Hover over an element",
        },
        {
          name: "hover-and-click",
          description: "Hover over one element, then click another",
        },
        {
          name: "select-option",
          description: "Select an option from a dropdown",
        },
        {
          name: "drag-and-drop",
          description: "Drag one element onto another",
        },
        {
          name: "cdp-click-element",
          description: "Click an element using CDP",
        },
        {
          name: "cdp-type-text",
          description: "Type text using CDP input injection",
        },
        {
          name: "go-back",
          description: "Navigate back in browser history",
        },
        {
          name: "go-forward",
          description: "Navigate forward in browser history",
        },
        {
          name: "refresh",
          description: "Reload the current page",
        },
        {
          name: "get-text",
          description: "Print the visible text of an element",
        },
        {
          name: "get-attribute",
          description: "Print an element attribute value",
        },
        {
          name: "get-property",
          description: "Print an element property value",
        },
        {
          name: "get-title",
          description: "Print the page title",
        },
        {
          name: "get-current-url",
          description: "Print the current URL",
        },
        {
          name: "clear-cookies",
          description: "Delete all browser cookies",
        },
        {
          name: "accept-alert",
          description: "Accept the active browser alert",
        },
        {
          name: "dismiss-alert",
          description: "Dismiss the active browser alert",
        },
        {
          name: "get-alert-text",
          description: "Print the text of the active alert",
        },
        {
          name: "type-alert-text",
          description: "Type text into the active alert prompt",
        },
        {
          name: "clear-local-storage",
          description: "Clear the page local storage",
        },
        {
          name: "get-local-storage-item",
          description: "Print a local storage value by key",
        },
        {
          name: "set-local-storage-item",
          description: "Set a local storage key/value pair",
        },
        {
          name: "remove-local-storage-item",
          description: "Remove a local storage entry by key",
        },
        {
          name: "switch-to-window",
          description: "Switch focus to a window by handle",
        },
        {
          name: "switch-to-frame",
          description: "Switch focus to an iframe",
        },
        {
          name: "switch-to-default-content",
          description: "Switch focus back to the main document",
        },
        {
          name: "get-cookies",
          description: "Print all browser cookies as JSON",
        },
        {
          name: "export-recording",
          description: "Export the recorded test actions",
        },
        {
          name: "patch-chromedriver",
          description:
            "Patch a chromedriver binary to reduce detection surface",
        },
        {
          name: "patch-chrome",
          description:
            "Patch a Chrome/Chromium binary for native-level spoofing",
        },
        {
          name: "encrypt",
          description:
            "Encrypt text with a passphrase (AES-256-GCM); read from stdin if TEXT is omitted",
        },
        {
          name: "decrypt",
          description:
            "Decrypt a token made by `sbrs encrypt`; read from stdin if TOKEN is omitted. Python's `sbrs unobfuscate` is the same command",
        },
        {
          name: "doctor",
          description:
            "Run a diagnostic check on the environment and configuration",
        },
        {
          name: "assert-text-visible",
          description: "Assert that an element contains the expected text",
        },
        {
          name: "assert-text-not-visible",
          description: "Assert that an element does not contain the given text",
        },
        {
          name: "assert-attribute",
          description: "Assert an element attribute equals an expected value",
        },
        {
          name: "assert-title",
          description: "Assert the page title contains the expected text",
        },
        {
          name: "wait-for-ready-state-complete",
          description: "Wait until the document readyState is complete",
        },
        {
          name: "get-window-position",
          description: "Print the browser window position",
        },
        {
          name: "set-window-position",
          description: "Set the browser window position",
        },
        {
          name: "close-window",
          description: "Close the current browser window",
        },
        {
          name: "switch-to-parent-frame",
          description: "Switch focus to the parent frame",
        },
        {
          name: "is-element-visible",
          description: "Print whether an element is visible",
        },
        {
          name: "is-text-visible",
          description:
            "Print whether the expected text is visible in an element",
        },
        {
          name: "wait-for-element-not-visible",
          description: "Wait until an element is no longer visible",
        },
        {
          name: "save-cookies",
          description: "Save browser cookies to a JSON file",
        },
        {
          name: "load-cookies",
          description: "Load browser cookies from a JSON file",
        },
        {
          name: "highlight-click",
          description: "Highlight an element, then click it",
        },
        {
          name: "is-checked",
          description: "Print whether a checkbox is checked",
        },
        {
          name: "check-if-unchecked",
          description: "Check a checkbox only if it is unchecked",
        },
        {
          name: "uncheck-if-checked",
          description: "Uncheck a checkbox only if it is checked",
        },
        {
          name: "open-new-window",
          description: "Open a new browser window",
        },
        {
          name: "open-new-tab",
          description: "Open a new browser tab",
        },
        {
          name: "switch-to-newest-window",
          description: "Switch focus to the newest browser window",
        },
        {
          name: "switch-to-default-window",
          description: "Switch focus to the default window",
        },
        {
          name: "get-active-element-css",
          description: "Print a CSS selector for the currently focused element",
        },
        {
          name: "wait-for-element-present",
          description: "Wait until an element is present in the DOM",
        },
        {
          name: "add-text",
          description: "Append text to an input or textarea",
        },
        {
          name: "send-keys",
          description: "Send keystrokes to an element",
        },
        {
          name: "get-value",
          description: "Print the value of a form element",
        },
        {
          name: "click-visible-elements",
          description: "Click all visible elements matching a selector",
        },
        {
          name: "wait-for-and-accept-alert",
          description: "Wait for an alert, then accept it",
        },
        {
          name: "wait-for-and-dismiss-alert",
          description: "Wait for an alert, then dismiss it",
        },
        {
          name: "is-link-text-visible",
          description: "Print whether a link with the given text is visible",
        },
        {
          name: "is-partial-link-text-visible",
          description:
            "Print whether a link containing the given text is visible",
        },
        {
          name: "assert-link-text",
          description: "Assert a link with the given text is visible",
        },
        {
          name: "click-partial-link-text",
          description: "Click a link containing the given text",
        },
        {
          name: "human-type",
          description: "Type text with human-like timing and noise",
        },
        {
          name: "human-click",
          description: "Click an element with human-like timing and noise",
        },
        {
          name: "smooth-scroll-to",
          description: "Smoothly scroll an element into view",
        },
        {
          name: "uc-click",
          description: "Perform a UC-mode click on an element",
        },
        {
          name: "uc-type",
          description: "Perform UC-mode typing into an element",
        },
        {
          name: "install",
          description:
            "Download a driver into ./downloaded_drivers/ (Python's `sbrs get`)",
        },
        {
          name: "test",
          description: "Run the project's tests, or one example, through Cargo",
        },
        {
          name: "grid-hub",
          description: "Start, stop, restart or check a Selenium Grid hub",
        },
        {
          name: "grid-node",
          description: "Start, stop, restart or check a Selenium Grid node",
        },
        {
          name: "translate",
          description:
            "Write the actions of a JSON scenario in another language",
        },
        {
          name: "mkdir",
          description: "Create a folder with a runnable browser-test suite",
        },
        {
          name: "mkfile",
          description: "Create a browser-test file",
        },
        {
          name: "commander",
          description:
            "Launch the interactive commander (needs a build with the `tui` feature)",
        },
        {
          name: "caseplans",
          description:
            "Launch the Case Plans Generator, a terminal app (needs a build with the `tui` feature)",
        },
        {
          name: "behave-gui",
          description:
            "Launch the Behave Commander, a terminal app (needs a build with the `tui` feature)",
        },
        {
          name: "print",
          description: "Print a file",
        },
        {
          name: "objectify",
          description: "Objectify a test recording",
        },
        {
          name: "mkpres",
          description: "Create an HTML presentation",
        },
        {
          name: "mkchart",
          description: "Create an HTML chart",
        },
        {
          name: "mkrec",
          description: "Create a test recording scaffold",
        },
        {
          name: "run-scenario",
          description: "Run a scenario file and write an optional dashboard",
        },
        {
          name: "completions",
          description:
            "Generate a completion script for a supported shell, or a Fig spec",
        },
        {
          name: "import-python",
          description:
            "Convert common SeleniumBase or Selenium Python code into a Rust test",
        },
        {
          name: "import-playwright",
          description:
            "Convert Playwright tests or a script (JavaScript, TypeScript or Python) into Rust tests for the pure CDP engine",
        },
        {
          name: "import-puppeteer",
          description:
            "Convert Puppeteer scripts or tests (JavaScript, TypeScript) or a Chrome Recorder JSON export into Rust tests for the pure CDP engine",
        },
        {
          name: "download-file",
          description:
            "Download a file from a URL (browser download with an HTTP fallback)",
        },
        {
          name: "print-pdf",
          description: "Print a page to PDF and save it to disk",
        },
        {
          name: "pdf-text",
          description: "Print the text extracted from a page or PDF URL",
        },
        {
          name: "assert-pdf-text",
          description:
            "Assert that a page or PDF URL contains the expected text",
        },
        {
          name: "choose-file",
          description: "Upload a local file into a file input",
        },
        {
          name: "deferred",
          description:
            "Run several assertions, then report every failure at once",
        },
        {
          name: "record",
          description:
            "Record browser interactions into a replayable action file",
        },
        {
          name: "replay",
          description:
            "Replay a recorded action file against a fresh browser session",
        },
        {
          name: "devtools",
          description:
            "Launch Chrome with an open DevTools endpoint and print MCP client configs",
        },
        {
          name: "help",
          description:
            "Print this message or the help of the given subcommand(s)",
        },
      ],
    },
  ],
  options: [
    {
      name: "--webdriver",
      description:
        "WebDriver server URL (default: SB_WEBDRIVER_URL, else http://localhost:4444)",
      isRepeatable: true,
      args: {
        name: "webdriver",
        isOptional: true,
      },
    },
    {
      name: "--browser",
      description:
        "Browser engine to launch (default: chrome, or the config file's)",
      isRepeatable: true,
      args: {
        name: "browser",
        isOptional: true,
        suggestions: [
          "chrome",
          "chromium",
          "edge",
          "firefox",
          {
            name: "brave",
            description: "Brave, run with chromedriver",
          },
          {
            name: "comet",
            description: "Perplexity's Comet, run with chromedriver",
          },
          {
            name: "opera",
            description: "Opera, run with operadriver",
          },
          {
            name: "safari",
            description: "Safari, run with safaridriver (macOS)",
          },
          {
            name: "ie",
            description: "Internet Explorer, run with IEDriverServer (Windows)",
          },
          {
            name: "cft",
            description: "Chrome for Testing, downloaded when missing",
          },
          {
            name: "chs",
            description: "chrome-headless-shell, downloaded when missing",
          },
        ],
      },
    },
    {
      name: "--user-agent",
      description: "Override the default user agent string",
      isRepeatable: true,
      args: {
        name: "user_agent",
        isOptional: true,
      },
    },
    {
      name: ["-a", "--agent"],
      description: "Alias for --user-agent",
      isRepeatable: true,
      args: {
        name: "agent",
        isOptional: true,
      },
    },
    {
      name: "--locale",
      description: "Locale string for the browser context",
      isRepeatable: true,
      args: {
        name: "locale",
        isOptional: true,
      },
    },
    {
      name: "--proxy",
      description:
        "Proxy (host:port or scheme://host:port); with --proxy-list, a name from the list",
      isRepeatable: true,
      args: {
        name: "proxy",
        isOptional: true,
      },
    },
    {
      name: "--proxy-list",
      description:
        "File of proxies to choose from; without --proxy a random one is used",
      isRepeatable: true,
      args: {
        name: "proxy_list",
        isOptional: true,
      },
    },
    {
      name: "--proxy-pac-url",
      description: "URL to a proxy auto-config (PAC) file",
      isRepeatable: true,
      args: {
        name: "proxy_pac_url",
        isOptional: true,
      },
    },
    {
      name: "--user-data-dir",
      description: "Path to a persistent browser user-data directory",
      isRepeatable: true,
      args: {
        name: "user_data_dir",
        isOptional: true,
      },
    },
    {
      name: "--extension-dir",
      description: "Path to an unpacked extension directory",
      isRepeatable: true,
      args: {
        name: "extension_dir",
        isOptional: true,
      },
    },
    {
      name: ["-n", "--threads"],
      description: "Number of parallel threads for test runs",
      isRepeatable: true,
      args: {
        name: "threads",
        isOptional: true,
      },
    },
    {
      name: ["-c", "--config"],
      description: "Path to a TOML/JSON settings file",
      isRepeatable: true,
      args: {
        name: "config",
        isOptional: true,
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
      name: "--ad-block",
      description:
        "Refuse the requests of ad and tracker hosts (Chromium WebDriver sessions)",
    },
    {
      name: ["--reuse-session", "--reuse_session", "--rs"],
      description:
        "Share browsers among the tests of the run (SB_REUSE_SESSION)",
    },
    {
      name: ["--rcs", "--reuse-class-session", "--reuse_class_session"],
      description:
        "Share a browser among the tests of one Rust module (SB_REUSE_CLASS_SESSION)",
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
      description:
        "Write recordings as pure CDP code (sb_cdp Page and Locator calls)",
    },
    {
      name: ["-h", "--help"],
      description: "Print help (see more with '--help')",
    },
    {
      name: ["-V", "--version"],
      description: "Print version",
    },
  ],
};

export default completion;
