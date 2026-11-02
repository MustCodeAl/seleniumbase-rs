// The page half of a trace (see trace.rs). It is added to every new document
// while a trace runs, and evaluated in the document that is open when the trace
// starts and at each capture, so it must be safe to run more than once.
//
// The engine never sends Runtime.enable, so the browser's console events are
// not available; this script keeps the console calls and uncaught errors of the
// document in a bounded buffer instead, and hands them over with a snapshot of
// the document when the Rust side asks (`__sbtrace.take`). Everything it
// returns is untrusted data once it has run in a page, and the Rust side treats
// it so.
(function () {
  'use strict';
  if (window.__sbtrace) {
    return;
  }
  var MAX_BUFFER = 500;
  var MAX_TEXT = 2000;
  var MAX_HTML = 524288;
  var buffer = [];
  var dropped = 0;

  function show(value) {
    try {
      if (typeof value === 'string') {
        return value;
      }
      if (value && typeof value === 'object') {
        if (typeof value.stack === 'string' && value.stack) {
          return value.stack;
        }
        var json = JSON.stringify(value);
        return typeof json === 'string' ? json : String(value);
      }
      return String(value);
    } catch (error) {
      return '[unprintable]';
    }
  }

  function push(level, parts) {
    if (buffer.length >= MAX_BUFFER) {
      dropped += 1;
      return;
    }
    var text = [];
    for (var i = 0; i < parts.length; i += 1) {
      text.push(show(parts[i]));
    }
    var joined = text.join(' ');
    if (joined.length > MAX_TEXT) {
      joined = joined.slice(0, MAX_TEXT) + '...';
    }
    buffer.push({ at: Date.now(), level: level, text: joined });
  }

  ['debug', 'log', 'info', 'warn', 'error'].forEach(function (level) {
    var original = console[level];
    if (typeof original !== 'function') {
      return;
    }
    console[level] = function () {
      try {
        push(level, arguments);
      } catch (error) {
        // Never let the trace change what the page's own call does.
      }
      return original.apply(this, arguments);
    };
  });

  window.addEventListener('error', function (event) {
    try {
      push('error', [
        'Uncaught ' + (event.message || 'error') + ' (' + (event.filename || '') + ':' +
          (event.lineno || 0) + ')'
      ]);
    } catch (error) {
      // Ignored, as above.
    }
  });
  window.addEventListener('unhandledrejection', function (event) {
    try {
      push('error', ['Unhandled rejection: ' + show(event.reason)]);
    } catch (error) {
      // Ignored, as above.
    }
  });

  // A serialised copy of the document that the viewer shows in a sandboxed
  // frame: no scripts, stylesheets inlined as <style> when the page lets us read
  // their rules, nothing that fetches or redirects, and no value of a field a
  // person types a secret into.
  function snapshot() {
    var root = document.documentElement;
    if (!root) {
      return { html: '', truncated: false };
    }
    var clone = root.cloneNode(true);
    var live = root.querySelectorAll('link[rel~="stylesheet"]');
    var copy = clone.querySelectorAll('link[rel~="stylesheet"]');
    for (var i = 0; i < live.length && i < copy.length; i += 1) {
      var css = null;
      try {
        var rules = live[i].sheet && live[i].sheet.cssRules;
        if (rules) {
          css = '';
          for (var j = 0; j < rules.length; j += 1) {
            css += rules[j].cssText + '\n';
          }
        }
      } catch (error) {
        css = null;
      }
      if (css !== null) {
        var style = document.createElement('style');
        style.textContent = css;
        copy[i].parentNode.replaceChild(style, copy[i]);
      } else {
        copy[i].parentNode.removeChild(copy[i]);
      }
    }
    var drop = clone.querySelectorAll('script, noscript, base, meta[http-equiv]');
    for (var k = 0; k < drop.length; k += 1) {
      if (drop[k].parentNode) {
        drop[k].parentNode.removeChild(drop[k]);
      }
    }
    var secrets = clone.querySelectorAll(
      'input[type="password" i], input[autocomplete*="password" i], input[autocomplete^="cc-" i]'
    );
    for (var m = 0; m < secrets.length; m += 1) {
      secrets[m].removeAttribute('value');
    }
    var html = '<!doctype html>' + clone.outerHTML;
    if (html.length > MAX_HTML) {
      return { html: html.slice(0, MAX_HTML), truncated: true };
    }
    return { html: html, truncated: false };
  }

  Object.defineProperty(window, '__sbtrace', {
    value: {
      take: function (withSnapshot) {
        var messages = buffer.splice(0, buffer.length);
        var lost = dropped;
        dropped = 0;
        var shot = withSnapshot ? snapshot() : { html: null, truncated: false };
        return {
          url: String(location.href),
          title: String(document.title),
          html: shot.html,
          truncated: shot.truncated,
          console: messages,
          dropped: lost
        };
      }
    },
    enumerable: false
  });
})();
