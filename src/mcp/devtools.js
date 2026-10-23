// SeleniumBase MCP developer tools: the page side.
//
// The file is one function expression. The server calls it as
// `(<this file>)(op, args)`, with `op` naming what to do and `args` an object
// of JSON values, and gets a JSON value back. It leaves nothing behind in the
// page, except the console recorder that `console` installs when asked to.
//
// Selectors arrive already tagged by the server, as
// `{ kind: "css" | "xpath" | "link_text" | "partial_link_text", value }`, the
// same rules both engines use, so a selector that works here works there.
// Every list in an answer is bounded, and says when it was cut short.
(function (op, args) {
  "use strict";

  // How many elements a walk of the page looks at before it stops.
  var MAX_NODES = 20000;
  // The most characters of text an answer repeats for one element.
  var TEXT_LIMIT = 80;
  // Attributes that tests hang their hooks on, best first.
  var TEST_ATTRIBUTES = [
    "data-testid", "data-test-id", "data-test", "data-qa", "data-cy", "data-pw",
    "data-automation-id", "data-e2e"
  ];
  var INTERACTIVE_ROLES = [
    "button", "link", "checkbox", "radio", "textbox", "searchbox", "combobox",
    "listbox", "slider", "spinbutton", "switch", "tab", "menuitem",
    "menuitemcheckbox", "menuitemradio", "option", "treeitem"
  ];

  // ------------------------------------------------------------------
  // Small helpers
  // ------------------------------------------------------------------

  function normal(text) {
    return String(text == null ? "" : text).replace(/\s+/g, " ").trim();
  }

  function clip(text, limit) {
    text = normal(text);
    limit = limit || TEXT_LIMIT;
    return text.length > limit ? text.slice(0, limit - 1) + "…" : text;
  }

  function quote(value) {
    return '"' + String(value).replace(/\\/g, "\\\\").replace(/"/g, '\\"') + '"';
  }

  function cssIdent(value) {
    if (window.CSS && CSS.escape) return CSS.escape(value);
    return String(value).replace(/[^a-zA-Z0-9_-]/g, function (c) { return "\\" + c; });
  }

  function xpathLiteral(value) {
    if (value.indexOf("'") === -1) return "'" + value + "'";
    if (value.indexOf('"') === -1) return '"' + value + '"';
    return "concat('" + value.split("'").join("', \"'\", '") + "')";
  }

  // Ids and classes that a framework or a build made up, and that change
  // from one build or one page load to the next.
  function looksGenerated(value) {
    return /\d{3,}/.test(value) ||
      /[0-9a-f]{6,}/i.test(value) && /\d/.test(value) ||
      /^(css|sc|jsx|emotion|ember|mui|chakra|svelte)-/i.test(value) ||
      /^:r[0-9a-z]+:$/.test(value) ||
      /__[a-zA-Z0-9]{5}$/.test(value);
  }

  // The Levenshtein distance between two short strings, for near misses.
  function distance(a, b) {
    if (Math.abs(a.length - b.length) > 3) return 99;
    var row = [];
    for (var j = 0; j <= b.length; j++) row.push(j);
    for (var i = 1; i <= a.length; i++) {
      var diagonal = row[0];
      row[0] = i;
      for (var k = 1; k <= b.length; k++) {
        var above = row[k];
        row[k] = a[i - 1] === b[k - 1] ? diagonal : 1 + Math.min(diagonal, above, row[k - 1]);
        diagonal = above;
      }
    }
    return row[b.length];
  }

  function similar(wanted, found) {
    if (wanted === found) return false;
    if (wanted.toLowerCase() === found.toLowerCase()) return true;
    var allowed = Math.max(1, Math.ceil(wanted.length / 4));
    return distance(wanted.toLowerCase(), found.toLowerCase()) <= Math.min(allowed, 3);
  }

  // ------------------------------------------------------------------
  // Elements
  // ------------------------------------------------------------------

  function isVisible(el) {
    if (!el || !el.isConnected) return false;
    var view = el.ownerDocument.defaultView;
    if (!view) return false;
    var style = view.getComputedStyle(el);
    if (style.display === "none") return false;
    if (style.visibility === "hidden" || style.visibility === "collapse") return false;
    if (parseFloat(style.opacity) === 0) return false;
    var r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0;
  }

  function isEnabled(el) {
    if (el.disabled) return false;
    if (el.getAttribute("aria-disabled") === "true") return false;
    var fieldset = el.closest && el.closest("fieldset[disabled]");
    return !fieldset;
  }

  function inViewport(el) {
    var r = el.getBoundingClientRect();
    var view = el.ownerDocument.defaultView;
    return r.bottom > 0 && r.right > 0 && r.top < view.innerHeight && r.left < view.innerWidth;
  }

  function inputType(el) {
    return (el.getAttribute("type") || "text").toLowerCase();
  }

  function roleOf(el) {
    var explicit = el.getAttribute("role");
    if (explicit) return explicit.split(/\s+/)[0];
    switch (el.localName) {
      case "a": return el.hasAttribute("href") ? "link" : "generic";
      case "button": case "summary": return "button";
      case "select": return el.multiple || el.size > 1 ? "listbox" : "combobox";
      case "textarea": return "textbox";
      case "img": return "img";
      case "h1": case "h2": case "h3": case "h4": case "h5": case "h6": return "heading";
      case "input": {
        var roles = {
          button: "button", submit: "button", reset: "button", image: "button",
          checkbox: "checkbox", radio: "radio", range: "slider", number: "spinbutton",
          search: "searchbox", hidden: "none"
        };
        return roles[inputType(el)] || "textbox";
      }
    }
    return el.isContentEditable ? "textbox" : "generic";
  }

  function isInteractive(el) {
    var tag = el.localName;
    if (tag === "input") return inputType(el) !== "hidden";
    if (tag === "button" || tag === "select" || tag === "textarea" || tag === "summary") return true;
    if (tag === "a" && el.hasAttribute("href")) return true;
    if (el.isContentEditable && !(el.parentElement && el.parentElement.isContentEditable)) return true;
    var role = el.getAttribute("role");
    if (role && INTERACTIVE_ROLES.indexOf(role.split(/\s+/)[0]) !== -1) return true;
    if (el.hasAttribute("onclick")) return true;
    var tabindex = el.getAttribute("tabindex");
    return tabindex !== null && Number(tabindex) >= 0 && tag !== "iframe" && tag !== "body";
  }

  function textOfIds(ids, root) {
    return ids.split(/\s+/).map(function (id) {
      var target = root.getElementById ? root.getElementById(id) : null;
      return target ? target.textContent : "";
    }).join(" ");
  }

  // The element's accessible name, roughly as a screen reader would say it.
  function nameOf(el) {
    var root = el.getRootNode();
    var labelledBy = el.getAttribute("aria-labelledby");
    if (labelledBy) {
      var byIds = clip(textOfIds(labelledBy, root));
      if (byIds) return byIds;
    }
    var aria = el.getAttribute("aria-label");
    if (aria && aria.trim()) return clip(aria);
    if (el.labels && el.labels.length) {
      var labels = clip(Array.prototype.map.call(el.labels, function (l) { return l.textContent; }).join(" "));
      if (labels) return labels;
    }
    var tag = el.localName;
    if (tag === "input") {
      var type = inputType(el);
      if (type === "submit" || type === "button" || type === "reset") return clip(el.value || type);
      if (type === "image") return clip(el.getAttribute("alt") || "");
    }
    if (tag === "img") return clip(el.getAttribute("alt") || "");
    if (tag !== "input" && tag !== "select" && tag !== "textarea") {
      var text = clip(el.textContent);
      if (text) return text;
      var img = el.querySelector("img[alt]");
      if (img && img.getAttribute("alt").trim()) return clip(img.getAttribute("alt"));
    }
    var title = el.getAttribute("title");
    if (title && title.trim()) return clip(title);
    var placeholder = el.getAttribute("placeholder");
    if (placeholder && placeholder.trim()) return clip(placeholder);
    return "";
  }

  // Calls `visit(el)` for every element under `root` and under the open
  // shadow roots inside it, in document order, until `visit` returns false or
  // MAX_NODES elements have been seen. Returns whether the walk was complete.
  function walk(root, visit, budget) {
    budget = budget || { left: MAX_NODES, stopped: false };
    var doc = root.nodeType === 9 ? root : root.ownerDocument;
    var walker = doc.createTreeWalker(root, NodeFilter.SHOW_ELEMENT);
    for (var node = walker.nextNode(); node; node = walker.nextNode()) {
      if (budget.stopped) break;
      if (budget.left-- <= 0) { budget.stopped = true; break; }
      if (visit(node) === false) { budget.stopped = true; break; }
      if (node.shadowRoot) walk(node.shadowRoot, visit, budget);
    }
    return !budget.stopped;
  }

  // Elements matching a tagged selector inside `root` (a document or a shadow
  // root). A selector the browser cannot parse throws.
  function query(selector, root) {
    switch (selector.kind) {
      case "css":
        return Array.prototype.slice.call(root.querySelectorAll(selector.value));
      case "xpath": {
        var doc = root.nodeType === 9 ? root : root.ownerDocument;
        var found = doc.evaluate(selector.value, root, null, XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null);
        var out = [];
        for (var i = 0; i < found.snapshotLength; i++) {
          var node = found.snapshotItem(i);
          if (node && node.nodeType === 1) out.push(node);
        }
        return out;
      }
      case "link_text": {
        var wanted = selector.value.trim();
        return Array.prototype.filter.call(root.querySelectorAll("a"), function (a) {
          return normal(a.textContent) === normal(wanted);
        });
      }
      case "partial_link_text":
        return Array.prototype.filter.call(root.querySelectorAll("a"), function (a) {
          return (a.textContent || "").indexOf(selector.value) !== -1;
        });
    }
    throw new Error("unknown selector kind " + selector.kind);
  }

  // A short description of an element, for a person: `button#buy.primary "Buy now"`.
  function describe(el) {
    var text = el.localName;
    if (el.id) text += "#" + el.id;
    var classes = (el.getAttribute("class") || "").trim().split(/\s+/).filter(Boolean).slice(0, 2);
    if (classes.length) text += "." + classes.join(".");
    var name = nameOf(el);
    return name ? text + " " + quote(clip(name, 40)) : text;
  }

  // ------------------------------------------------------------------
  // Selectors for an element
  // ------------------------------------------------------------------

  function count(selector, root) {
    try { return query(selector, root); } catch (e) { return []; }
  }

  // An `nth-of-type` path from `el` up to the nearest ancestor with a usable
  // id, or to the root.
  function cssPath(el) {
    var parts = [];
    for (var node = el; node && node.nodeType === 1; node = node.parentElement) {
      if (node !== el && node.id && !looksGenerated(node.id)) {
        parts.unshift("#" + cssIdent(node.id));
        break;
      }
      var tag = node.localName;
      var index = 1;
      var same = 0;
      for (var sib = node.parentElement ? node.parentElement.firstElementChild : null; sib; sib = sib.nextElementSibling) {
        if (sib.localName === tag) {
          same++;
          if (sib === node) index = same;
        }
      }
      parts.unshift(same > 1 ? tag + ":nth-of-type(" + index + ")" : tag);
      if (tag === "body") break;
    }
    return parts.join(" > ");
  }

  // Ranked selectors for `el`, each checked against the page: how many
  // elements it matches, and whether the first of them is `el`.
  function selectorsFor(el, limit) {
    var root = el.getRootNode();
    var tag = el.localName;
    var candidates = [];
    function add(kind, value, strategy, score, stable) {
      if (!value) return;
      for (var i = 0; i < candidates.length; i++) {
        if (candidates[i].selector === value) return;
      }
      var selector = { kind: kind, value: value };
      var found = count(selector, root);
      var shown = kind === "link_text" ? "link=" + value : value;
      candidates.push({
        selector: shown,
        strategy: strategy,
        matches: found.length,
        unique: found.length === 1 && found[0] === el,
        finds_element: found.length > 0 && found[0] === el,
        stable: stable,
        score: score,
      });
    }

    TEST_ATTRIBUTES.forEach(function (name) {
      var value = el.getAttribute(name);
      if (value) {
        add("css", "[" + name + "=" + quote(value) + "]", "test attribute", 100, true);
        add("css", tag + "[" + name + "=" + quote(value) + "]", "test attribute", 99, true);
      }
    });
    if (el.id) {
      var generated = looksGenerated(el.id);
      add("css", "#" + cssIdent(el.id), "id", generated ? 40 : 90, !generated);
    }
    var name = el.getAttribute("name");
    if (name) add("css", tag + "[name=" + quote(name) + "]", "name", 80, true);
    var aria = el.getAttribute("aria-label");
    if (aria && aria.trim()) add("css", tag + "[aria-label=" + quote(aria) + "]", "aria-label", 70, true);
    var placeholder = el.getAttribute("placeholder");
    if (placeholder && placeholder.trim()) {
      add("css", tag + "[placeholder=" + quote(placeholder) + "]", "placeholder", 60, true);
    }
    var text = normal(el.textContent);
    if (tag === "a" && el.hasAttribute("href") && text && text.length <= 60) {
      add("link_text", text, "link text", 65, true);
    }
    if (text && text.length <= 60 && el.childElementCount <= 3 && root.nodeType === 9) {
      add("xpath", "//" + tag + "[normalize-space()=" + xpathLiteral(text) + "]", "text", 55, true);
    }
    if (tag === "input" && el.value && /^(submit|button|reset)$/.test(inputType(el))) {
      add("css", "input[type=" + quote(inputType(el)) + "][value=" + quote(el.value) + "]", "value", 50, true);
    }
    var classes = (el.getAttribute("class") || "").trim().split(/\s+/).filter(function (c) {
      return c && !looksGenerated(c);
    }).slice(0, 2);
    if (classes.length) {
      add("css", tag + "." + classes.map(cssIdent).join("."), "class", 30, true);
    }
    add("css", cssPath(el), "position", 20, false);

    candidates.sort(function (a, b) {
      if (a.unique !== b.unique) return a.unique ? -1 : 1;
      if (a.finds_element !== b.finds_element) return a.finds_element ? -1 : 1;
      return b.score - a.score;
    });
    return candidates.slice(0, limit).map(function (c) {
      delete c.score;
      return c;
    });
  }

  // The best single selector for `el`, or "" when none finds it.
  function bestSelector(el) {
    var best = selectorsFor(el, 1)[0];
    return best && best.finds_element ? best.selector : "";
  }

  function shadowHost(el) {
    var root = el.getRootNode();
    return root && root.host ? root.host : null;
  }

  // ------------------------------------------------------------------
  // Operations
  // ------------------------------------------------------------------

  function pageFacts() {
    return {
      url: location.href,
      title: document.title,
      ready_state: document.readyState,
    };
  }

  function state() {
    var facts = pageFacts();
    if (args.selector) {
      var report = { value: args.raw || args.selector.value };
      try {
        var found = query(args.selector, document);
        report.matches = found.length;
        report.visible = found.filter(isVisible).length;
      } catch (e) {
        report.error = String(e && e.message || e);
      }
      facts.selector = report;
    }
    var store = window.__sbDevConsole;
    if (store) {
      facts.javascript_errors = store.entries.filter(function (entry) {
        return entry.level === "error";
      }).slice(-5).map(function (entry) { return entry.text; });
    }
    return facts;
  }

  function scopeRoot() {
    if (!args.scope) return { root: document.documentElement };
    var found;
    try {
      found = query(args.scope, document);
    } catch (e) {
      return { error: "the scope selector is not valid: " + (e && e.message || e) };
    }
    if (!found.length) return { error: "the scope selector matched nothing" };
    return { root: found[0] };
  }

  function inspect() {
    var limit = args.limit || 50;
    var includeHidden = !!args.include_hidden;
    var scope = scopeRoot();
    if (scope.error) return { error: scope.error };
    var root = scope.root;
    var out = pageFacts();
    out.viewport = {
      width: window.innerWidth, height: window.innerHeight,
      scroll_x: Math.round(window.scrollX), scroll_y: Math.round(window.scrollY),
    };
    var counts = { elements: 0, interactive: 0, links: 0, buttons: 0, inputs: 0, forms: 0,
      images: 0, iframes: 0, shadow_hosts: 0, hidden_interactive: 0 };
    var headings = [], forms = [], interactive = [], frames = [], hosts = [];
    var complete = walk(root, function (el) {
      counts.elements++;
      var tag = el.localName;
      if (/^h[1-6]$/.test(tag)) {
        if (headings.length < 30) headings.push({ level: Number(tag[1]), text: clip(el.textContent) });
      } else if (tag === "form") {
        counts.forms++;
        if (forms.length < 20) {
          forms.push({
            selector: bestSelector(el),
            action: el.getAttribute("action") || "",
            method: (el.getAttribute("method") || "get").toLowerCase(),
            fields: el.elements ? el.elements.length : 0,
          });
        }
      } else if (tag === "img") {
        counts.images++;
      } else if (tag === "iframe" || tag === "frame") {
        counts.iframes++;
        if (frames.length < 20) {
          var sameOrigin = false;
          try { sameOrigin = !!el.contentDocument; } catch (e) { sameOrigin = false; }
          frames.push({ selector: bestSelector(el), src: clip(el.getAttribute("src") || "", 200), same_origin: sameOrigin });
        }
      }
      if (el.shadowRoot) {
        counts.shadow_hosts++;
        if (hosts.length < 20) hosts.push({ selector: bestSelector(el), tag: tag });
      }
      if (!isInteractive(el)) return true;
      counts.interactive++;
      var role = roleOf(el);
      if (role === "link") counts.links++;
      else if (role === "button") counts.buttons++;
      else counts.inputs++;
      var shown = isVisible(el);
      if (!shown) counts.hidden_interactive++;
      if (!shown && !includeHidden) return true;
      if (interactive.length >= limit) return true;
      var entry = {
        role: role,
        name: nameOf(el),
        tag: tag,
        selector: bestSelector(el),
        visible: shown,
        enabled: isEnabled(el),
      };
      if (tag === "input") entry.type = inputType(el);
      if (shown && !inViewport(el)) entry.off_screen = true;
      var host = shadowHost(el);
      if (host) entry.in_shadow_root_of = bestSelector(host) || describe(host);
      interactive.push(entry);
      return true;
    });
    out.counts = counts;
    out.headings = headings;
    out.forms = forms;
    out.interactive = interactive;
    out.iframes = frames;
    out.shadow_hosts = hosts;
    var listed = includeHidden ? counts.interactive : counts.interactive - counts.hidden_interactive;
    out.truncated = !complete || interactive.length < listed;
    if (out.truncated) {
      out.note = !complete
        ? "the page has more than " + MAX_NODES + " elements; only the first were read"
        : "listed " + interactive.length + " of " + listed + " controls; raise limit or set scope";
    }
    return out;
  }

  // Elements a person might mean by `text` and `role`, best first.
  function findByText(text, role, max) {
    var wanted = normal(text).toLowerCase();
    var scored = [];
    walk(document.documentElement, function (el) {
      var tag = el.localName;
      if (tag === "script" || tag === "style" || tag === "head" || tag === "html" || tag === "body") return true;
      var interactive = isInteractive(el);
      if (role && roleOf(el) !== role) return true;
      if (!wanted) {
        if (interactive || role) scored.push({ el: el, score: 1 + (isVisible(el) ? 2 : 0) });
        return scored.length < 500;
      }
      if (!interactive && el.childElementCount > 0 && !/^(label|h[1-6]|p|li|td|th|legend|summary|option|span|dt|dd)$/.test(tag)) {
        return true;
      }
      var name = normal(interactive ? nameOf(el) : el.textContent).toLowerCase();
      if (!name || name.length > 300) return true;
      var score = 0;
      if (name === wanted) score = 6;
      else if (name.indexOf(wanted) !== -1) score = 3;
      else if (el.getAttribute("placeholder") && normal(el.getAttribute("placeholder")).toLowerCase() === wanted) score = 5;
      else if (el.getAttribute("value") && normal(el.getAttribute("value")).toLowerCase() === wanted) score = 4;
      if (!score) return true;
      if (interactive) score += 2;
      if (isVisible(el)) score += 1;
      scored.push({ el: el, score: score });
      return scored.length < 500;
    });
    // A container that holds the text through a child that also matched is
    // not what was meant: keep the innermost of the best.
    scored.sort(function (a, b) { return b.score - a.score; });
    var picked = [];
    for (var i = 0; i < scored.length && picked.length < max; i++) {
      var el = scored[i].el;
      var inner = picked.some(function (p) { return el.contains(p) || p.contains(el); });
      if (!inner) picked.push(el);
    }
    return picked;
  }

  function suggest() {
    var limit = args.limit || 5;
    var targets;
    if (args.selector) {
      try {
        targets = query(args.selector, document).slice(0, 3);
      } catch (e) {
        targets = [];
      }
    } else {
      targets = findByText(args.text || "", args.role || "", 3);
    }
    return {
      url: location.href,
      elements: targets.map(function (el) {
        var entry = {
          element: describe(el),
          role: roleOf(el),
          visible: isVisible(el),
          enabled: isEnabled(el),
          selectors: selectorsFor(el, limit),
        };
        var host = shadowHost(el);
        if (host) {
          entry.in_shadow_root_of = bestSelector(host) || describe(host);
          entry.note = "selectors cannot reach into a shadow root; these work from inside it only";
        }
        return entry;
      }),
    };
  }

  function details(el) {
    var view = el.ownerDocument.defaultView;
    var style = view.getComputedStyle(el);
    var r = el.getBoundingClientRect();
    var out = {
      element: describe(el),
      selector: bestSelector(el),
      visible: isVisible(el),
      enabled: isEnabled(el),
      display: style.display,
      visibility: style.visibility,
      opacity: Number(style.opacity),
      size: { width: Math.round(r.width), height: Math.round(r.height) },
      in_viewport: inViewport(el),
    };
    if (out.visible && out.in_viewport) {
      var x = Math.min(Math.max(r.left + r.width / 2, 0), view.innerWidth - 1);
      var y = Math.min(Math.max(r.top + r.height / 2, 0), view.innerHeight - 1);
      var top = el.ownerDocument.elementFromPoint(x, y);
      if (top && top !== el && !el.contains(top) && !top.contains(el)) {
        out.covered_by = describe(top);
        var cover = bestSelector(top);
        if (cover) out.covered_by_selector = cover;
      }
    }
    return out;
  }

  function nearMisses(selector) {
    var misses = [];
    function note(kind, wanted, found, suggestion, matches) {
      if (misses.length < 5) misses.push({ kind: kind, wanted: wanted, found: found, try: suggestion, matches: matches });
    }
    var ids = [], classes = {}, attributes = {};
    walk(document.documentElement, function (el) {
      if (el.id && ids.length < 5000) ids.push(el.id);
      var cls = el.getAttribute("class");
      if (cls) cls.trim().split(/\s+/).forEach(function (c) { classes[c] = true; });
      for (var i = 0; i < el.attributes.length; i++) {
        var a = el.attributes[i];
        if (a.name.indexOf("data-") === 0 || a.name === "name" || a.name === "aria-label") {
          var key = a.name + "=" + a.value;
          if (!attributes[key]) attributes[key] = { name: a.name, value: a.value };
        }
      }
      return true;
    });
    if (selector.kind === "css") {
      var value = selector.value;
      (value.match(/#[A-Za-z0-9_-]+/g) || []).forEach(function (token) {
        var id = token.slice(1);
        ids.filter(function (found) { return similar(id, found); }).slice(0, 2).forEach(function (found) {
          var fixed = value.split(token).join("#" + cssIdent(found));
          note("id", id, found, fixed, count({ kind: "css", value: fixed }, document).length);
        });
      });
      (value.match(/\.[A-Za-z0-9_-]+/g) || []).forEach(function (token) {
        var cls = token.slice(1);
        Object.keys(classes).filter(function (found) { return similar(cls, found); }).slice(0, 2).forEach(function (found) {
          var fixed = value.split(token).join("." + cssIdent(found));
          note("class", cls, found, fixed, count({ kind: "css", value: fixed }, document).length);
        });
      });
      var attribute = /\[\s*([A-Za-z_:-][A-Za-z0-9_:.-]*)\s*[~|^$*]?=\s*["']?([^"'\]]*)["']?\s*\]/g;
      var m;
      while ((m = attribute.exec(value)) !== null) {
        var wantedName = m[1], wantedValue = m[2];
        Object.keys(attributes).forEach(function (key) {
          var a = attributes[key];
          var sameName = a.name === wantedName;
          var nameNear = !sameName && a.value === wantedValue && (a.name.indexOf("data-") === 0 || similar(wantedName, a.name));
          var valueNear = sameName && similar(wantedValue, a.value);
          if (nameNear || valueNear) {
            var fixed = "[" + a.name + "=" + quote(a.value) + "]";
            note("attribute", wantedName + "=" + wantedValue, a.name + "=" + a.value, fixed,
              count({ kind: "css", value: fixed }, document).length);
          }
        });
      }
    }
    // Text the selector looks for, written differently on the page.
    var texts = [];
    if (selector.kind === "link_text" || selector.kind === "partial_link_text") texts.push(selector.value);
    (selector.value.match(/"([^"]{2,80})"|'([^']{2,80})'/g) || []).forEach(function (q) {
      texts.push(q.slice(1, -1));
    });
    texts.forEach(function (text) {
      var wanted = normal(text).toLowerCase();
      if (!wanted) return;
      findByText(text, "", 3).forEach(function (el) {
        var actual = normal(el.textContent);
        if (actual === normal(text) || actual.length > 80) return;
        if (actual.toLowerCase() !== wanted && actual.toLowerCase().indexOf(wanted) === -1) return;
        var suggestion = el.localName === "a" && el.hasAttribute("href")
          ? "link=" + actual
          : "//" + el.localName + "[normalize-space()=" + xpathLiteral(actual) + "]";
        note("text", text, actual, suggestion, 1);
      });
    });
    return misses;
  }

  function diagnose() {
    var selector = args.selector;
    var out = pageFacts();
    out.selector = args.raw || selector.value;
    out.kind = selector.kind;
    var found;
    try {
      found = query(selector, document);
    } catch (e) {
      out.syntax_error = String(e && e.message || e);
      out.matches = 0;
      return out;
    }
    out.matches = found.length;
    out.visible_matches = found.filter(isVisible).length;
    out.elements = found.slice(0, 5).map(details);
    if (found.length === 0) {
      var frames = [];
      var shadows = [];
      walk(document.documentElement, function (el) {
        var tag = el.localName;
        if ((tag === "iframe" || tag === "frame") && frames.length < 10) {
          var doc = null;
          try { doc = el.contentDocument; } catch (e) { doc = null; }
          var entry = { frame: bestSelector(el) || describe(el), src: clip(el.getAttribute("src") || "", 200), same_origin: !!doc };
          if (doc) {
            try { entry.matches = query(selector, doc).length; } catch (e) { entry.matches = 0; }
          }
          frames.push(entry);
        }
        if (el.shadowRoot && selector.kind !== "xpath" && shadows.length < 10) {
          var inside = count(selector, el.shadowRoot).length;
          if (inside) shadows.push({ host: bestSelector(el) || describe(el), matches: inside });
        }
        return true;
      });
      out.frames = frames;
      out.shadow_roots = shadows;
      out.near_misses = nearMisses(selector);
    }
    return out;
  }

  function installConsole() {
    var store = { entries: [], dropped: 0, since: new Date().toISOString() };
    function keep(level, text) {
      if (store.entries.length >= 500) { store.entries.shift(); store.dropped++; }
      store.entries.push({ level: level, text: clip(text, 500), time: new Date().toISOString() });
    }
    function show(value) {
      if (typeof value === "string") return value;
      if (value instanceof Error) return value.name + ": " + value.message;
      try { return JSON.stringify(value); } catch (e) { return String(value); }
    }
    ["debug", "log", "info", "warn", "error"].forEach(function (level) {
      var original = console[level];
      console[level] = function () {
        keep(level === "warn" ? "warning" : level, Array.prototype.map.call(arguments, show).join(" "));
        return original.apply(this, arguments);
      };
    });
    window.addEventListener("error", function (event) {
      keep("error", "Uncaught " + (event.message || (event.error && event.error.message) || "error"));
    });
    window.addEventListener("unhandledrejection", function (event) {
      var reason = event.reason;
      keep("error", "Unhandled promise rejection: " + show(reason && reason.message || reason));
    });
    Object.defineProperty(window, "__sbDevConsole", { value: store, enumerable: false, configurable: true });
    return store;
  }

  function consoleMessages() {
    var store = window.__sbDevConsole;
    var installed = false;
    if (!store && args.install) {
      store = installConsole();
      installed = true;
    }
    if (!store) return { recording: false, entries: [] };
    var levels = args.levels || null;
    var entries = store.entries.filter(function (entry) {
      return !levels || levels.indexOf(entry.level) !== -1;
    });
    var limit = args.limit || 100;
    var out = {
      recording: true,
      started_now: installed,
      since: store.since,
      total: entries.length,
      dropped: store.dropped,
      entries: entries.slice(-limit),
    };
    if (args.clear) {
      store.entries.length = 0;
      store.dropped = 0;
    }
    return out;
  }

  function failedResources() {
    var supported = typeof PerformanceResourceTiming !== "undefined" &&
      "responseStatus" in PerformanceResourceTiming.prototype;
    var entries = performance.getEntriesByType("navigation").concat(performance.getEntriesByType("resource"));
    var failed = [];
    var unknown = 0;
    entries.forEach(function (e) {
      var status = e.responseStatus || 0;
      if (status >= 400) {
        failed.push({ url: clip(e.name, 300), status: status, type: e.initiatorType || "navigation" });
      } else if (!status) {
        unknown++;
      }
    });
    return {
      supported: supported,
      checked: entries.length,
      status_unknown: unknown,
      failed: failed.slice(0, args.limit || 100),
    };
  }

  function source() {
    // A copy of the page with the values of password and hidden inputs
    // removed: a bug report must not carry a session's secrets.
    var copy = document.documentElement.cloneNode(true);
    var blanked = 0;
    Array.prototype.forEach.call(copy.querySelectorAll("input[type=password], input[type=hidden]"), function (input) {
      if (input.hasAttribute("value")) {
        input.setAttribute("value", "[redacted]");
        blanked++;
      }
    });
    var doctype = document.doctype ? "<!DOCTYPE " + document.doctype.name + ">\n" : "";
    return { html: doctype + copy.outerHTML, redacted_inputs: blanked };
  }

  function environment() {
    var keys = function (storage) {
      try { return Object.keys(storage).slice(0, 200); } catch (e) { return []; }
    };
    return {
      url: location.href,
      title: document.title,
      ready_state: document.readyState,
      user_agent: navigator.userAgent,
      language: navigator.language,
      viewport: { width: window.innerWidth, height: window.innerHeight, device_pixel_ratio: window.devicePixelRatio },
      local_storage_keys: keys(window.localStorage),
      session_storage_keys: keys(window.sessionStorage),
    };
  }

  // ------------------------------------------------------------------
  // Page parsing: snapshot, outline, text, links, tables and forms
  // ------------------------------------------------------------------

  var SKIP_TAGS = {
    script: 1, style: 1, noscript: 1, template: 1, head: 1, meta: 1, link: 1, title: 1, base: 1
  };
  var NON_TEXT = { svg: 1, canvas: 1, iframe: 1, frame: 1, object: 1, embed: 1, select: 1, option: 1 };
  var BLOCK_TAGS = {
    address: 1, article: 1, aside: 1, body: 1, dd: 1, details: 1, dialog: 1, div: 1, dl: 1, dt: 1,
    fieldset: 1, figcaption: 1, figure: 1, footer: 1, form: 1, header: 1, hr: 1, html: 1, li: 1,
    main: 1, nav: 1, p: 1, section: 1, summary: 1, tr: 1
  };
  var LEAF_ROLES = {
    heading: 1, link: 1, button: 1, textbox: 1, searchbox: 1, checkbox: 1, radio: 1, "switch": 1,
    slider: 1, spinbutton: 1, combobox: 1, listbox: 1, img: 1, option: 1, tab: 1, menuitem: 1,
    menuitemcheckbox: 1, menuitemradio: 1, treeitem: 1, progressbar: 1, meter: 1, iframe: 1
  };
  var CONTAINER_ROLES = {
    main: 1, navigation: 1, banner: 1, contentinfo: 1, complementary: 1, region: 1, form: 1,
    search: 1, article: 1, dialog: 1, alertdialog: 1, table: 1, grid: 1, group: 1, list: 1,
    tablist: 1, toolbar: 1, menu: 1, menubar: 1, tree: 1, radiogroup: 1, alert: 1, status: 1
  };
  var LANDMARKS = {
    main: 1, navigation: 1, banner: 1, contentinfo: 1, complementary: 1, region: 1, search: 1
  };
  var NAMED_ONLY = { region: 1, group: 1 };
  var CHROME_TAGS = { nav: 1, header: 1, footer: 1, aside: 1 };
  var CHROME_ROLES = { navigation: 1, banner: 1, contentinfo: 1, complementary: 1 };
  var SECRET_AUTOCOMPLETE = /^(cc-|current-password|new-password|one-time-code)/;

  // A field whose value must never be printed: passwords, card data, codes.
  function isSecretField(el) {
    if (el.localName !== "input" && el.localName !== "textarea") return false;
    if (inputType(el) === "password") return true;
    return SECRET_AUTOCOMPLETE.test((el.getAttribute("autocomplete") || "").toLowerCase());
  }

  function isKnownRole(role) {
    return !!(LEAF_ROLES[role] || CONTAINER_ROLES[role]);
  }

  function sectioned(el) {
    return !!(el.parentElement && el.parentElement.closest("article, aside, main, nav, section"));
  }

  // The name an author gave the element with aria-labelledby or aria-label.
  function ariaName(el) {
    var labelledBy = el.getAttribute("aria-labelledby");
    if (labelledBy) {
      var byIds = clip(textOfIds(labelledBy, el.getRootNode()));
      if (byIds) return byIds;
    }
    var aria = el.getAttribute("aria-label");
    return aria && aria.trim() ? clip(aria) : "";
  }

  // The role the snapshot shows for an element, or "" when it is not shown.
  function snapRole(el) {
    var explicit = el.getAttribute("role");
    if (explicit) {
      var first = explicit.trim().split(/\s+/)[0].toLowerCase();
      return isKnownRole(first) ? first : "";
    }
    switch (el.localName) {
      case "main": return "main";
      case "nav": return "navigation";
      case "header": return sectioned(el) ? "" : "banner";
      case "footer": return sectioned(el) ? "" : "contentinfo";
      case "aside": return "complementary";
      case "form": return "form";
      case "section": return ariaName(el) ? "region" : "";
      case "article": return "article";
      case "dialog": return "dialog";
      case "table": return "table";
      case "iframe": case "frame": return "iframe";
      case "fieldset": return ariaName(el) || el.querySelector("legend") ? "group" : "";
      case "img": return (el.getAttribute("alt") || "").trim() ? "img" : "";
      case "progress": return "progressbar";
      case "meter": return "meter";
      case "option": return "";
      case "summary": return "button";
    }
    var base = roleOf(el);
    return isKnownRole(base) ? base : "";
  }

  function legendOrCaption(el) {
    var node = null;
    if (el.localName === "table") node = el.querySelector("caption");
    else if (el.localName === "fieldset") node = el.querySelector("legend");
    else if (el.localName === "details") node = el.querySelector("summary");
    return node ? clip(node.textContent) : "";
  }

  // The name the snapshot shows: a landmark or container is named by its
  // author, never by the text inside it.
  function snapName(el, role) {
    if (CONTAINER_ROLES[role]) {
      var own = ariaName(el);
      if (own) return own;
      if (role === "form") return clip(el.getAttribute("name") || el.getAttribute("title") || "");
      return legendOrCaption(el) || clip(el.getAttribute("title") || "");
    }
    if (role === "iframe") return clip(el.getAttribute("title") || el.getAttribute("name") || "");
    return nameOf(el);
  }

  function hiddenSubtree(el) {
    if (el.hidden || el.getAttribute("aria-hidden") === "true" || el.hasAttribute("inert")) return true;
    return el.ownerDocument.defaultView.getComputedStyle(el).display === "none";
  }

  function hasTestAttribute(el) {
    return TEST_ATTRIBUTES.some(function (name) { return el.hasAttribute(name); });
  }

  // `/html[1]/body[1]/div[2]/a[1]`: always one element, in a document.
  function xpathIndexed(el) {
    var parts = [];
    for (var node = el; node && node.nodeType === 1; node = node.parentNode) {
      var index = 1;
      for (var sib = node.previousElementSibling; sib; sib = sib.previousElementSibling) {
        if (sib.localName === node.localName) index++;
      }
      parts.unshift(node.localName + "[" + index + "]");
    }
    return "/" + parts.join("/");
  }

  // A selector that matches `el` and nothing else inside `root`, or "".
  function uniqueIn(el, root) {
    var found = selectorsFor(el, 8);
    for (var i = 0; i < found.length; i++) {
      if (found[i].unique) return found[i].selector;
    }
    if (root.nodeType === 9) {
      var path = xpathIndexed(el);
      var list = count({ kind: "xpath", value: path }, root);
      if (list.length === 1 && list[0] === el) return path;
    }
    return "";
  }

  // The selector a ref stands for. An element in an open shadow root is
  // reached through its hosts: `host ::shadow inner`, as the WebDriver engine
  // reads it. "" when no selector finds exactly this element.
  function refSelector(el) {
    var parts = [];
    var node = el;
    for (var guard = 0; node && guard < 10; guard++) {
      var root = node.getRootNode();
      var one = uniqueIn(node, root);
      if (!one) return "";
      parts.unshift(one);
      if (root.nodeType === 11 && root.host) node = root.host;
      else return parts.join(" ::shadow ");
    }
    return "";
  }

  // The role and name a ref is checked against later.
  function roleName(el) {
    var role = snapRole(el) || "generic";
    return { role: role, name: snapName(el, role) };
  }

  function addRef(entry, el) {
    var selector = refSelector(el);
    if (!selector) return;
    var facts = roleName(el);
    entry.selector = selector;
    entry.ref_role = facts.role;
    entry.ref_name = facts.name;
  }

  function refable(role, name) {
    return INTERACTIVE_ROLES.indexOf(role) !== -1 || role === "form" || role === "iframe" ||
      (!!LANDMARKS[role] && !!name);
  }

  // What the snapshot prints next to a node.
  function roleFacts(el, role) {
    var f = {};
    var tag = el.localName;
    if (role === "heading") {
      var level = /^h([1-6])$/.exec(tag);
      f.level = level ? Number(level[1]) : Number(el.getAttribute("aria-level")) || 2;
    }
    if (el.required || el.getAttribute("aria-required") === "true") f.required = true;
    if (el.readOnly && (tag === "input" || tag === "textarea")) f.readonly = true;
    if (role === "checkbox" || role === "radio" || role === "switch" ||
        role === "menuitemcheckbox" || role === "menuitemradio") {
      if (tag === "input") {
        f.checked = !!el.checked;
      } else {
        var checked = el.getAttribute("aria-checked");
        f.checked = checked === "true" ? true : checked === "mixed" ? "mixed" : false;
      }
    }
    var expanded = el.getAttribute("aria-expanded");
    if (expanded === "true" || expanded === "false") f.expanded = expanded === "true";
    if (el.getAttribute("aria-selected") === "true" || (tag === "option" && el.selected)) f.selected = true;
    if (el.getAttribute("aria-pressed") === "true") f.pressed = true;
    var current = el.getAttribute("aria-current");
    if (current && current !== "false") f.current = current;
    if ((tag === "input" || tag === "textarea") && !isSecretField(el)) {
      var type = tag === "input" ? inputType(el) : "text";
      if (!/^(submit|button|reset|image|checkbox|radio|hidden|file)$/.test(type) && el.value) {
        f.value = clip(el.value, 80);
      }
    } else if (tag === "select") {
      var chosen = Array.prototype.filter.call(el.options, function (o) { return o.selected; })
        .map(function (o) { return normal(o.text); });
      if (chosen.length) f.value = clip(chosen.join(", "), 80);
    }
    if (tag === "a" && role === "link" && el.hasAttribute("href")) f.href = clip(el.href, 160);
    if (role === "iframe" && el.getAttribute("src")) f.src = clip(el.getAttribute("src"), 160);
    return f;
  }

  // snapshot (the accessibility tree) and outline (the tag tree): a flat list
  // of nodes in document order, each with its depth; the server draws it.
  function snapshot(outline) {
    var maxDepth = args.depth || 12;
    var maxNodes = args.max_nodes || 400;
    var includeHidden = !!args.include_hidden;
    var scope = scopeRoot();
    if (scope.error) return { error: scope.error };
    var out = pageFacts();
    var nodes = [];
    var state = { more: 0, seen: 0, cut: false };

    function childrenOf(el) {
      if (el.localName === "slot" && el.assignedElements) {
        var assigned = el.assignedElements({ flatten: true });
        if (assigned.length) return assigned;
      }
      return Array.prototype.slice.call((el.shadowRoot || el).children);
    }

    function entryFor(el, role, depth) {
      var shown = role || "generic";
      var entry = {
        d: depth, role: shown, name: snapName(el, shown), tag: el.localName,
        visible: isVisible(el), enabled: isEnabled(el)
      };
      if (el.getRootNode().nodeType === 11) entry.shadow = true;
      var wantsRef;
      if (outline) {
        if (el.id) entry.id = el.id;
        var classes = (el.getAttribute("class") || "").trim().split(/\s+/).filter(Boolean).slice(0, 2);
        if (classes.length) entry.cls = classes.join(".");
        entry.children = el.children.length;
        entry.text_len = (el.textContent || "").length;
        wantsRef = !!el.id || hasTestAttribute(el);
      } else {
        var facts = roleFacts(el, shown);
        for (var key in facts) entry[key] = facts[key];
        wantsRef = refable(shown, entry.name);
      }
      if (wantsRef) {
        var selector = refSelector(el);
        if (selector) entry.selector = selector;
      }
      return entry;
    }

    function visit(el, depth) {
      if (state.seen++ >= MAX_NODES) { state.cut = true; return; }
      if (SKIP_TAGS[el.localName]) return;
      if (!includeHidden && hiddenSubtree(el)) return;
      var role = snapRole(el);
      var wanted = outline || (!!role && (!NAMED_ONLY[role] || !!snapName(el, role)));
      var childDepth = depth;
      if (wanted && (includeHidden || isVisible(el))) {
        if (depth >= maxDepth || nodes.length >= maxNodes) {
          state.more++;
        } else {
          nodes.push(entryFor(el, role, depth));
          childDepth = depth + 1;
        }
      }
      if (!outline && wanted && LEAF_ROLES[role]) return;
      var kids = childrenOf(el);
      for (var i = 0; i < kids.length; i++) visit(kids[i], childDepth);
    }

    visit(scope.root, 0);
    out.mode = outline ? "outline" : "accessibility";
    out.nodes = nodes;
    out.more = state.more;
    out.truncated = state.more > 0 || state.cut;
    if (state.cut) out.note = "the page has more than " + MAX_NODES + " elements; only the first were read";
    return out;
  }

  // The element still has the role and name it had? Answers for the last
  // part of a (possibly shadow-piercing) selector given as tagged parts.
  function verifyRef() {
    var parts = args.parts || [];
    var context = document;
    var found = [];
    for (var i = 0; i < parts.length; i++) {
      var list;
      try {
        list = query(parts[i], context);
      } catch (e) {
        return { count: 0, url: location.href, error: String(e && e.message || e) };
      }
      if (i === parts.length - 1) { found = list; break; }
      if (!list.length || !list[0].shadowRoot) return { count: 0, url: location.href };
      context = list[0].shadowRoot;
    }
    var out = { count: found.length, url: location.href, role: "", name: "" };
    if (found.length) {
      var facts = roleName(found[0]);
      out.role = facts.role;
      out.name = facts.name;
    }
    return out;
  }

  // Slot content, the shadow root of a host, or the plain child nodes.
  function kidsOf(node) {
    if (node.localName === "slot" && node.assignedNodes) {
      var assigned = node.assignedNodes({ flatten: true });
      if (assigned.length) return assigned;
    }
    return (node.shadowRoot || node).childNodes;
  }

  // ---- tables -------------------------------------------------------

  function rowList(table) {
    if (table.localName === "table") {
      return Array.prototype.map.call(table.rows, function (tr) {
        return {
          cells: Array.prototype.slice.call(tr.cells),
          header: tr.parentNode.localName === "thead"
        };
      });
    }
    return Array.prototype.map.call(table.querySelectorAll("[role=row]"), function (row) {
      var cells = Array.prototype.slice.call(
        row.querySelectorAll("[role=columnheader], [role=rowheader], [role=cell], [role=gridcell]"));
      return {
        cells: cells,
        header: cells.length > 0 && cells.every(function (c) {
          return c.getAttribute("role") === "columnheader";
        })
      };
    });
  }

  function spanFill(row, spans, col) {
    while (spans[col] && spans[col].left > 0) {
      row[col] = spans[col].text;
      spans[col].left--;
      col++;
    }
    return col;
  }

  // The cells of a table with colspan and rowspan expanded.
  function tableGrid(table, maxRows) {
    var list = rowList(table);
    var spans = [];
    var rows = [];
    var headerRows = [];
    var truncated = false;
    for (var r = 0; r < list.length; r++) {
      if (rows.length >= maxRows + 1) { truncated = true; break; }
      var row = [];
      var col = 0;
      var allHeader = list[r].cells.length > 0;
      for (var k = 0; k < list[r].cells.length; k++) {
        col = spanFill(row, spans, col);
        var cell = list[r].cells[k];
        var text = clip(cell.textContent, 200);
        var role = cell.getAttribute("role");
        if (cell.localName !== "th" && role !== "columnheader" && role !== "rowheader") allHeader = false;
        var cs = Math.min(Math.max(cell.colSpan || 1, 1), 50);
        var rs = Math.min(Math.max(cell.rowSpan || 1, 1), 100);
        for (var j = 0; j < cs; j++) {
          row[col] = text;
          if (rs > 1) spans[col] = { left: rs - 1, text: text };
          col++;
        }
      }
      spanFill(row, spans, col);
      for (var c = 0; c < row.length; c++) {
        if (row[c] === undefined) row[c] = "";
      }
      rows.push(row);
      headerRows.push(list[r].header || allHeader);
    }
    var headers = [];
    var hasHeader = rows.length > 0 && headerRows[0];
    if (hasHeader) headers = rows.shift();
    var total = list.length - (hasHeader ? 1 : 0);
    if (rows.length > maxRows) { rows = rows.slice(0, maxRows); truncated = true; }
    return { headers: headers, rows: rows, total: total, truncated: truncated };
  }

  function tableText(table, markdown) {
    var grid = tableGrid(table, 100);
    var rows = grid.headers.length ? [grid.headers].concat(grid.rows) : grid.rows;
    if (!rows.length) return "";
    var width = 0;
    rows.forEach(function (row) { width = Math.max(width, row.length); });
    var lines = rows.map(function (row) {
      var cells = [];
      for (var c = 0; c < width; c++) cells.push((row[c] || "").replace(/\|/g, "\\|"));
      return markdown ? "| " + cells.join(" | ") + " |" : cells.join(" | ");
    });
    if (markdown) {
      var rule = [];
      for (var c = 0; c < width; c++) rule.push("---");
      lines.splice(1, 0, "| " + rule.join(" | ") + " |");
    }
    return lines.join("\n");
  }

  function tablesOp() {
    var scope = scopeRoot();
    if (scope.error) return { error: scope.error };
    var limit = args.limit || 20;
    var maxRows = args.max_rows || 200;
    var wanted = "table, [role=table], [role=grid]";
    var found = Array.prototype.slice.call(scope.root.querySelectorAll(wanted));
    if (scope.root.matches && scope.root.matches(wanted)) found.unshift(scope.root);
    var tables = [];
    var more = 0;
    for (var i = 0; i < found.length; i++) {
      var el = found[i];
      if (!isVisible(el)) continue;
      if (tables.length >= limit) { more++; continue; }
      var grid = tableGrid(el, maxRows);
      var entry = {
        headers: grid.headers, rows: grid.rows, row_count: grid.total, truncated: grid.truncated
      };
      var caption = snapName(el, snapRole(el) || "table");
      if (caption) entry.caption = caption;
      addRef(entry, el);
      tables.push(entry);
    }
    return { url: location.href, title: document.title, tables: tables, more_tables: more };
  }

  // ---- forms --------------------------------------------------------

  function fieldType(el) {
    if (el.localName === "input") return inputType(el);
    if (el.localName === "select") return el.multiple ? "select-multiple" : "select";
    return el.localName;
  }

  function formsOp() {
    var scope = scopeRoot();
    if (scope.error) return { error: scope.error };
    var list = Array.prototype.slice.call(scope.root.querySelectorAll("form"));
    if (scope.root.localName === "form") list.unshift(scope.root);
    var forms = [];
    var fieldBudget = 200;
    var more = 0;
    for (var i = 0; i < list.length; i++) {
      if (forms.length >= 20) { more++; continue; }
      var form = list[i];
      var entry = {
        action: form.getAttribute("action") ? form.action : "",
        method: (form.getAttribute("method") || "get").toLowerCase(),
        fields: []
      };
      addRef(entry, form);
      var submit = null;
      for (var k = 0; k < form.elements.length; k++) {
        var el = form.elements[k];
        var tag = el.localName;
        if (tag !== "input" && tag !== "select" && tag !== "textarea" && tag !== "button") continue;
        var type = fieldType(el);
        if (/^(submit|button|reset|image)$/.test(type)) {
          if (!submit && type !== "reset" && type !== "button") submit = el;
          continue;
        }
        if (tag === "button") { if (!submit) submit = el; continue; }
        if (fieldBudget-- <= 0) { entry.truncated = true; break; }
        var field = {
          name: el.name || el.id || "", type: type, label: nameOf(el), required: !!el.required
        };
        if (type === "checkbox" || type === "radio") field.checked = !!el.checked;
        if (!isSecretField(el) && el.value && type !== "file") field.value = clip(el.value, 200);
        if (tag === "select") {
          field.options = Array.prototype.slice.call(el.options, 0, 50).map(function (o) {
            return { value: o.value, label: normal(o.text), selected: o.selected };
          });
        }
        addRef(field, el);
        entry.fields.push(field);
      }
      if (submit) {
        entry.submit = { label: nameOf(submit) };
        addRef(entry.submit, submit);
      }
      forms.push(entry);
    }
    return { url: location.href, title: document.title, forms: forms, more_forms: more };
  }

  // ---- links --------------------------------------------------------

  function regionOf(el) {
    for (var node = el.parentElement; node; node = node.parentElement) {
      var role = snapRole(node);
      if (LANDMARKS[role] || role === "dialog" || role === "form") return role;
    }
    return "body";
  }

  function linksOp() {
    var scope = scopeRoot();
    if (scope.error) return { error: scope.error };
    var limit = args.limit || 200;
    var sameOrigin = !!args.same_origin;
    var contains = String(args.contains || "").toLowerCase();
    var anchors = scope.root.querySelectorAll("a[href], area[href]");
    var seen = {};
    var links = [];
    var total = 0;
    for (var i = 0; i < anchors.length; i++) {
      var a = anchors[i];
      if (!isVisible(a)) continue;
      var href = a.href;
      if (sameOrigin && a.origin !== location.origin) continue;
      var image = a.querySelector && a.querySelector("img[alt]");
      var text = clip(normal(a.textContent) || a.getAttribute("aria-label") || a.getAttribute("title") ||
        (image ? image.getAttribute("alt") : ""), 120);
      if (contains && (text + " " + href).toLowerCase().indexOf(contains) === -1) continue;
      var key = href + "\u0000" + text;
      if (seen[key] !== undefined) {
        if (seen[key] >= 0) links[seen[key]].count = (links[seen[key]].count || 1) + 1;
        continue;
      }
      total++;
      if (links.length >= limit) { seen[key] = -1; continue; }
      seen[key] = links.length;
      var entry = { text: text, href: href, region: regionOf(a) };
      var rel = a.getAttribute("rel");
      if (rel) entry.rel = rel;
      var target = a.getAttribute("target");
      if (target) entry.target = target;
      if (/^javascript:/i.test(href)) entry.kind = "script";
      if (links.length < 100) addRef(entry, a);
      links.push(entry);
    }
    return {
      url: location.href, title: document.title, links: links, total: total,
      truncated: total > links.length
    };
  }

  // ---- readable text ------------------------------------------------

  function densest(root) {
    var current = root;
    for (var depth = 0; depth < 12; depth++) {
      var total = (current.textContent || "").length;
      var best = null;
      var bestLength = 0;
      for (var i = 0; i < current.children.length; i++) {
        var child = current.children[i];
        if (SKIP_TAGS[child.localName]) continue;
        var length = (child.textContent || "").length;
        if (length > bestLength) { best = child; bestLength = length; }
      }
      if (!best || bestLength < 200 || bestLength < total * 0.6) break;
      current = best;
    }
    return current;
  }

  function textBlocks(root, markdown, skipChrome) {
    var blocks = [];
    var line = "";

    function flush() {
      var t = line.replace(/\s+/g, " ").trim();
      if (t) blocks.push(t);
      line = "";
    }

    // The visible text under `node` on one line, links and code marked up.
    function inline(node) {
      if (node.nodeType === 3) return node.nodeValue;
      if (node.nodeType !== 1) return "";
      var tag = node.localName;
      if (SKIP_TAGS[tag] || NON_TEXT[tag] || hiddenSubtree(node)) return "";
      if (tag === "br") return " ";
      var text = "";
      var kids = kidsOf(node);
      for (var i = 0; i < kids.length; i++) text += inline(kids[i]);
      if (tag === "a" && markdown) {
        var label = normal(text);
        var href = node.href;
        if (label && href && !/^javascript:/i.test(href)) return "[" + label + "](" + href + ")";
      }
      if (tag === "code" && markdown && normal(text)) return "`" + normal(text) + "`";
      return text;
    }

    function listLines(list, depth, ordered, lines) {
      var index = 0;
      Array.prototype.forEach.call(list.children, function (li) {
        if (li.localName !== "li" || hiddenSubtree(li)) return;
        index++;
        var own = "";
        var nested = [];
        Array.prototype.forEach.call(li.childNodes, function (child) {
          if (child.nodeType === 1 && (child.localName === "ul" || child.localName === "ol")) nested.push(child);
          else own += inline(child);
        });
        var text = normal(own);
        if (text) lines.push(new Array(depth + 1).join("  ") + (ordered ? index + ". " : "- ") + text);
        nested.forEach(function (inner) { listLines(inner, depth + 1, inner.localName === "ol", lines); });
      });
    }

    function walkNode(node) {
      if (node.nodeType === 3) { line += node.nodeValue; return; }
      if (node.nodeType !== 1) return;
      var tag = node.localName;
      if (SKIP_TAGS[tag] || NON_TEXT[tag] || hiddenSubtree(node)) return;
      if (skipChrome && (CHROME_TAGS[tag] || CHROME_ROLES[node.getAttribute("role") || ""])) return;
      if (/^h[1-6]$/.test(tag)) {
        flush();
        var heading = normal(inline(node));
        if (heading) blocks.push(markdown ? "######".slice(0, Number(tag[1])) + " " + heading : heading);
        return;
      }
      if (tag === "a" || tag === "code") { line += inline(node); return; }
      if (tag === "br") { flush(); return; }
      if (tag === "pre") {
        flush();
        var raw = (node.textContent || "").replace(/\s+$/, "");
        if (raw) blocks.push(markdown ? "```\n" + raw + "\n```" : raw);
        return;
      }
      if (tag === "ul" || tag === "ol") {
        flush();
        var lines = [];
        listLines(node, 0, tag === "ol", lines);
        if (lines.length) blocks.push(lines.join("\n"));
        return;
      }
      if (tag === "table") {
        flush();
        var table = tableText(node, markdown);
        if (table) blocks.push(table);
        return;
      }
      if (tag === "blockquote") {
        flush();
        var quote_ = normal(inline(node));
        if (quote_) blocks.push((markdown ? "> " : "") + quote_);
        return;
      }
      var block = !!BLOCK_TAGS[tag];
      if (block) flush();
      var kids = kidsOf(node);
      for (var i = 0; i < kids.length; i++) walkNode(kids[i]);
      if (block) flush();
    }

    walkNode(root);
    flush();
    return blocks;
  }

  function textOp() {
    var scope = scopeRoot();
    if (scope.error) return { error: scope.error };
    var markdown = args.format !== "text";
    var mainOnly = args.main_only !== false;
    var maxChars = args.max_chars || 20000;
    var root = scope.root;
    var source = args.scope ? "scope" : "page";
    var skipChrome = false;
    if (mainOnly && !args.scope) {
      var main = document.querySelector("main, [role=main]");
      var articles = document.querySelectorAll("article");
      if (main) {
        root = main;
        source = "main";
      } else if (articles.length === 1) {
        root = articles[0];
        source = "article";
      } else {
        var body = document.body || root;
        var dense = densest(body);
        root = dense;
        if (dense !== body) source = "densest block";
        else skipChrome = true;
      }
    }
    var blocks = textBlocks(root, markdown, skipChrome);
    var kept = [];
    var used = 0;
    var total = 0;
    blocks.forEach(function (block) { total += block.length + 2; });
    total = Math.max(0, total - 2);
    var cut = false;
    for (var i = 0; i < blocks.length; i++) {
      var next = used + (kept.length ? 2 : 0) + blocks[i].length;
      if (next > maxChars) {
        cut = true;
        if (!kept.length) { kept.push(blocks[i].slice(0, maxChars)); used = maxChars; }
        break;
      }
      kept.push(blocks[i]);
      used = next;
    }
    var text = kept.join("\n\n");
    var remaining = Math.max(0, total - used);
    if (cut) text += "\n\n... [cut: " + remaining + " more characters]";
    return {
      text: text, chars: used, total_chars: total, truncated: cut, cut_chars: cut ? remaining : 0,
      source: source, format: markdown ? "markdown" : "text", url: location.href, title: document.title
    };
  }


  switch (op) {
    case "state": return state();
    case "inspect": return inspect();
    case "suggest": return suggest();
    case "diagnose": return diagnose();
    case "console": return consoleMessages();
    case "resources": return failedResources();
    case "source": return source();
    case "environment": return environment();
    case "snapshot": return snapshot(false);
    case "outline": return snapshot(true);
    case "verify_ref": return verifyRef();
    case "text": return textOp();
    case "links": return linksOp();
    case "tables": return tablesOp();
    case "forms": return formsOp();
  }
  throw new Error("unknown developer-tools operation " + op);
})
