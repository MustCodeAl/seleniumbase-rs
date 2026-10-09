// SeleniumBase Pure CDP page helpers.
//
// Injected into every document by `Page.addScriptToEvaluateOnNewDocument` and
// evaluated once into the current one. Everything hangs off `window.__sbcdp`,
// so repeated injection is a no-op.
//
// A locator is a *chain* of steps, resolved left to right. Each step finds
// elements inside every element the previous step produced, optionally keeps
// only the visible ones, and optionally narrows to one by position:
//
//   { selector: { kind, value }, visible: bool, nth: number | null, last: bool }
//
// `kind` is decided by the Rust side; nothing here guesses it from the text.
// Errors whose message starts with `sbcdp:` are turned into typed errors.
(function () {
  if (window.__sbcdp) return;

  function xpath(expression, scope) {
    // An absolute path such as "//a" or "(//a)[1]" ignores the context node and
    // searches the whole document, so it must be confined to the scope.
    // Relative ones ("./a", "..", "parent::div") start at the scope already and
    // may legitimately lead out of it.
    const absolute = /^\s*\(*\s*\//.test(expression);
    const out = [];
    const snapshot = document.evaluate(
      expression, scope, null, XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null);
    for (let i = 0; i < snapshot.snapshotLength; i++) {
      const node = snapshot.snapshotItem(i);
      if (!node || node.nodeType !== 1) continue;
      if (absolute && scope !== document && !scope.contains(node)) continue;
      out.push(node);
    }
    return out;
  }

  function query(selector, scope) {
    switch (selector.kind) {
      case "css":
        return Array.from(scope.querySelectorAll(selector.value));
      case "xpath":
        return xpath(selector.value, scope);
      case "link_text": {
        const wanted = selector.value.trim();
        return Array.from(scope.querySelectorAll("a"))
          .filter((a) => (a.textContent || "").trim() === wanted);
      }
      case "partial_link_text":
        return Array.from(scope.querySelectorAll("a"))
          .filter((a) => (a.textContent || "").includes(selector.value));
      default:
        throw new Error("sbcdp:bad-selector:" + selector.kind);
    }
  }

  function visible(el) {
    if (!el || !el.isConnected) return false;
    const style = getComputedStyle(el);
    if (style.display === "none") return false;
    if (style.visibility === "hidden" || style.visibility === "collapse") return false;
    if (parseFloat(style.opacity) === 0) return false;
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0;
  }

  // Every element the chain currently matches, in document order per step.
  function resolve(chain) {
    let scopes = [document];
    for (const step of chain) {
      const found = new Set();
      for (const scope of scopes) {
        for (const el of query(step.selector, scope)) found.add(el);
      }
      let matches = Array.from(found);
      if (step.visible) matches = matches.filter(visible);
      if (step.last) matches = matches.slice(-1);
      else if (step.nth !== null && step.nth !== undefined) matches = matches.slice(step.nth, step.nth + 1);
      scopes = matches;
    }
    return scopes;
  }

  function one(chain) {
    const els = resolve(chain);
    if (els.length === 0) throw new Error("sbcdp:not-found");
    return els[0];
  }

  function textOf(el) {
    return el.innerText !== undefined ? el.innerText : (el.textContent || "");
  }

  function attrs(el) {
    const out = {};
    for (const a of el.attributes) out[a.name] = a.value;
    return out;
  }

  function rectOf(el) {
    const r = el.getBoundingClientRect();
    return { x: r.left + window.scrollX, y: r.top + window.scrollY,
             width: r.width, height: r.height };
  }

  function info(el) {
    return {
      tag: el.tagName.toLowerCase(),
      text: textOf(el),
      html: el.outerHTML,
      attributes: attrs(el),
      rect: rectOf(el),
      visible: visible(el),
    };
  }

  // Scrolls an element to the middle of the viewport and returns the viewport
  // coordinates of its centre, ready for synthetic mouse events.
  function center(el) {
    el.scrollIntoView({ block: "center", inline: "center", behavior: "instant" });
    const r = el.getBoundingClientRect();
    if (r.width === 0 && r.height === 0) {
      throw new Error("sbcdp:not-interactable:the element has no size");
    }
    const x = r.left + r.width / 2;
    const y = r.top + r.height / 2;
    const top = document.elementFromPoint(x, y);
    return { x, y, covered: !!top && top !== el && !el.contains(top) };
  }

  // Sets a value through the native setter, so controlled inputs in React and
  // similar frameworks notice it, then fires the events a user's edit would.
  function setValue(el, value) {
    const tag = el.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") {
      const proto = tag === "INPUT" ? HTMLInputElement.prototype
        : tag === "TEXTAREA" ? HTMLTextAreaElement.prototype
        : HTMLSelectElement.prototype;
      const desc = Object.getOwnPropertyDescriptor(proto, "value");
      if (desc && desc.set) desc.set.call(el, value); else el.value = value;
    } else if (el.isContentEditable) {
      el.textContent = value;
    } else {
      el.value = value;
    }
    el.dispatchEvent(new Event("input", { bubbles: true }));
    el.dispatchEvent(new Event("change", { bubbles: true }));
  }

  function focus(el) {
    el.scrollIntoView({ block: "center", inline: "center", behavior: "instant" });
    el.focus();
  }

  function selectOption(el, by, wanted) {
    if (el.tagName !== "SELECT") throw new Error("sbcdp:not-interactable:the element is not a <select>");
    const options = Array.from(el.options);
    let index = -1;
    if (by === "text") index = options.findIndex((o) => o.text.trim() === String(wanted).trim());
    else if (by === "value") index = options.findIndex((o) => o.value === String(wanted));
    else if (by === "index") index = Number(wanted) < options.length ? Number(wanted) : -1;
    if (index < 0) throw new Error("sbcdp:no-option:" + by + "=" + wanted);
    el.selectedIndex = index;
    el.dispatchEvent(new Event("input", { bubbles: true }));
    el.dispatchEvent(new Event("change", { bubbles: true }));
  }

  // Briefly outlines an element, then restores its previous style.
  function flash(el, color, millis) {
    const prevOutline = el.style.outline;
    const prevOffset = el.style.outlineOffset;
    el.style.outline = "4px solid " + (color || "#FFC107");
    el.style.outlineOffset = "2px";
    setTimeout(() => {
      el.style.outline = prevOutline;
      el.style.outlineOffset = prevOffset;
    }, millis || 800);
  }

  window.__sbcdp = {
    resolve, one, visible, info, center, setValue, focus, selectOption, flash,
    textOf, rectOf, attrs,
  };
})();
