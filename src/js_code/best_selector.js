// Returns the element a click on `target` is a click on: the button when
// `target` is the icon or label (`span`, `i`) inside one, `target` itself
// otherwise. `sbBestSelector` names this element, so whatever records the
// element alongside that selector should use it too.
function sbClickTarget(target) {
  if (!target || target.nodeType !== 1) return target;
  if ((target.localName === 'span' || target.localName === 'i') && target.parentElement) {
    var parent = target.parentElement;
    var grandparent = parent.parentElement;
    if (parent.localName === 'button') return parent;
    if (grandparent && grandparent.localName === 'button') return grandparent;
  }
  return target;
}

// Returns a short CSS selector for `target` (see `sbClickTarget`), or "" when
// `target` is not an element.
//
// The selector is built the way SeleniumBase's own `active_css_js` and
// recorder do it, so recorded and reported selectors look alike:
//
//   1. `tag#id` when the id is usable and the id picks the element out;
//   2. otherwise the shortest of the id, class and attribute paths (the
//      attributes tried are the ones tests and frameworks tend to hang a
//      hook on: name, data-qa, data-testid, aria-label, ...);
//   3. a lone `h1`/`input`/`textarea`..., or `tag:contains("text")` for a link,
//      button or label whose text is not repeated on the page.
//
// Unlike the original, nothing is returned that does not find the element: a
// candidate is used only if `document.querySelector` gives back `target`, and
// when none does the answer is the full `:nth-of-type` path from `<html>`,
// which always does. (`:contains()` is SeleniumBase's own pseudo-class, not
// CSS, so those answers cannot be checked this way.)
function sbBestSelector(target) {
  if (!target || target.nodeType !== 1) return '';
  var el = sbClickTarget(target);

  var ATTRIBUTES = [
    'name', 'data-qa', 'data-tid', 'data-el', 'data-se', 'data-name',
    'data-auto', 'data-text', 'data-test', 'data-testid', 'data-test-id',
    'data-test-selector', 'data-nav', 'data-sb', 'data-cy', 'data-action',
    'data-target', 'data-tooltip', 'alt', 'title', 'heading', 'translate',
    'aria-label', 'aria-describedby', 'rel', 'ng-model', 'ng-href', 'href',
    'label', 'data-content', 'data-tip', 'data-for', 'class', 'for',
    'placeholder', 'value', 'ng-click', 'ng-if', 'src'
  ];
  var BASIC_TAGS = ['h1', 'h2', 'h3', 'canvas', 'center', 'input', 'textarea'];
  var TEXT_TAGS = [
    'a', 'b', 'h1', 'h2', 'h3', 'h4', 'h5', 'code', 'mark', 'button', 'label',
    'legend', 'li', 'td', 'th', 'i', 'small', 'strong', 'summary', 'span'
  ];

  function quote(value) {
    return '"' + value.replace(/\\/g, '\\\\').replace(/"/g, '\\"') + '"';
  }
  function all(selector) {
    try { return document.querySelectorAll(selector); } catch (e) { return []; }
  }
  function depth(selector) { return selector.split(' > ').length - 1; }
  function isGenerated(text) { return /[_-]\d/.test(text) || /\d[a-z]/.test(text); }

  function identifies(selector) {
    var found = all(selector);
    return found.length > 0 && found[0] === el;
  }
  function nth(node, always) {
    var tag = node.localName;
    var index = 1;
    for (var sib = node.previousElementSibling; sib; sib = sib.previousElementSibling) {
      if (sib.localName === tag) index++;
    }
    return index !== 1 || always ? tag + ':nth-of-type(' + index + ')' : tag;
  }
  // The path from `el` up to the first ancestor `stop` returns a part for, or
  // up to the root when there is none.
  function pathUp(stop) {
    var parts = [];
    for (var node = el; node && node.nodeType === 1; node = node.parentNode) {
      var part = stop(node);
      if (part) {
        parts.unshift(part);
        break;
      }
      parts.unshift(nth(node, false));
    }
    return parts.join(' > ');
  }
  function byId(node) {
    var id = node.id;
    if (!id || id.indexOf('\n') !== -1) return null;
    var plain = /^[A-Za-z_][A-Za-z0-9_-]*$/.test(id);
    return node.localName + (plain ? '#' + id : '[id=' + quote(id) + ']');
  }
  function byAttribute(name) {
    return function (node) {
      var value = node.getAttribute(name);
      if (!value || value.indexOf('\n') !== -1) return null;
      return node.localName + '[' + name + '=' + quote(value) + ']';
    };
  }
  function byClass(node) {
    var value = node.getAttribute('class');
    if (!value || value.indexOf(' ') !== -1 || value.indexOf('-') === -1) return null;
    var part = node.localName + '.' + value;
    return all(part).length === 1 ? part : null;
  }

  var selectorById = pathUp(byId);
  if (selectorById.indexOf(' > ') === -1 && !isGenerated(selectorById) &&
      identifies(selectorById)) {
    return selectorById;
  }

  var candidates = [];
  if (identifies(selectorById)) {
    candidates.push({ selector: selectorById, depth: depth(selectorById) });
  }
  var selectorByClass = pathUp(byClass);
  for (var i = 0; i < ATTRIBUTES.length; i++) {
    var attribute = ATTRIBUTES[i];
    var selectorByAttribute = attribute === 'class'
      ? selectorByClass : pathUp(byAttribute(attribute));
    if (!identifies(selectorByAttribute)) continue;
    if (selectorByAttribute.indexOf(' > ') === -1) {
      var skip = (attribute.indexOf('aria') === 0 || attribute === 'for') &&
        /\d/.test(selectorByAttribute);
      if (!skip) return selectorByAttribute;
    }
    candidates.push({ selector: selectorByAttribute, depth: depth(selectorByAttribute) });
  }

  var tag = el.localName;
  if (BASIC_TAGS.indexOf(tag) !== -1) {
    var same = all(tag);
    if (same.length === 1 && same[0] === el) return tag;
  }
  var text = (el.textContent || '').trim();
  if (TEXT_TAGS.indexOf(tag) !== -1 && text.length >= 2 && text.length <= 64 &&
      text.indexOf('\n') === -1) {
    var repeats = 0;
    var sameTag = all(tag);
    for (var j = 0; j < sameTag.length; j++) {
      if (sameTag[j].textContent.indexOf(text) !== -1) repeats++;
    }
    if (repeats === 1) return tag + ':contains(' + quote(text) + ')';
  }

  var best = null;
  var bestDepth = Infinity;
  for (var k = 0; k < candidates.length; k++) {
    if (candidates[k].depth < bestDepth) {
      best = candidates[k].selector;
      bestDepth = candidates[k].depth;
    }
  }
  if (best === null) {
    var absolute = [];
    for (var node = el; node && node.nodeType === 1; node = node.parentNode) {
      absolute.unshift(nth(node, true));
    }
    return absolute.join(' > ');
  }
  best = best.split('html > body').join('body');
  var shorter = best.split(' > ').join(' ').split(' div ').join(' ');
  return identifies(shorter) ? shorter : best;
}
