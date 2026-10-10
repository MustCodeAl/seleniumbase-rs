// Records what happens in the page into `window.recordedActions`.
//
// Installing it twice in the same document is harmless: the second call finds
// the first one running, changes nothing and returns the same list. The list
// is also kept in `sessionStorage`, so installing it again after the next page
// of the same site has loaded carries on where the last page left off.
//
// Each entry is `{type, target, tag, selector, url, time}`, plus `value` for a
// change and `checked` for a box or radio button. `type` is `open` (the page
// the recording started or continued on), `click`, `dblclick`, `change` or
// `submit`. Typed passwords are not stored: such an entry has `sensitive: true`
// and an empty value.
//
// `window.__sbRecorder.stop()` removes the listeners.
if (window.__sbRecorder) return window.recordedActions;

var STORE = 'sb_recorder_actions';
var actions = [];
try {
  var saved = JSON.parse(window.sessionStorage.getItem(STORE) || '[]');
  if (Array.isArray(saved)) actions = saved;
} catch (error) {
  // No storage here (an opaque origin, or blocked): record in memory only.
}
window.recordedActions = actions;

function persist() {
  try {
    window.sessionStorage.setItem(STORE, JSON.stringify(actions));
  } catch (error) {
    // See above.
  }
}

function push(entry) {
  entry.url = window.location.href;
  entry.time = Date.now();
  actions.push(entry);
  persist();
}

function record(type, target, extra) {
  if (!target || target.nodeType !== 1) return;
  // The selector names the button for a click on the icon or label inside it,
  // so the tag recorded beside it must be the button's.
  var el = sbClickTarget(target);
  var entry = {
    type: type,
    target: el.tagName,
    tag: el.localName,
    selector: sbBestSelector(el)
  };
  for (var key in extra) entry[key] = extra[key];
  push(entry);
}

function onClick(event) { record('click', event.target, {}); }
function onDoubleClick(event) { record('dblclick', event.target, {}); }
function onSubmit(event) { record('submit', event.target, {}); }
function onChange(event) {
  var el = event.target;
  if (!el || el.nodeType !== 1) return;
  if (el.type === 'checkbox' || el.type === 'radio') {
    record('change', el, { checked: el.checked });
  } else if (el.type === 'password') {
    record('change', el, { value: '', sensitive: true });
  } else if (el.localName === 'select') {
    var chosen = el.selectedOptions && el.selectedOptions[0];
    record('change', el, { value: chosen ? chosen.text : el.value });
  } else {
    record('change', el, { value: el.value });
  }
}

var last = actions.length ? actions[actions.length - 1] : null;
if (!last || last.url !== window.location.href) push({ type: 'open' });

// Capturing listeners, so a page that stops an event from bubbling cannot hide
// it from the recording.
document.addEventListener('click', onClick, true);
document.addEventListener('dblclick', onDoubleClick, true);
document.addEventListener('change', onChange, true);
document.addEventListener('submit', onSubmit, true);

window.__sbRecorder = {
  stop: function () {
    document.removeEventListener('click', onClick, true);
    document.removeEventListener('dblclick', onDoubleClick, true);
    document.removeEventListener('change', onChange, true);
    document.removeEventListener('submit', onSubmit, true);
    delete window.__sbRecorder;
  }
};
return actions;
