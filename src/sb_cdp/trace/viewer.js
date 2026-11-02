// The trace viewer: draws the data in <script id="trace-data"> and nothing else.
//
// Every string in that data came from a page or a caller and is untrusted. This
// script therefore builds the page with createElement and textContent only: no
// innerHTML, no document.write, no eval. The one place a page's HTML is used is
// the DOM snapshot, which goes into an iframe whose sandbox attribute is empty
// (no scripts, no forms, no popups, no same-origin access) and which the
// page's Content-Security-Policy, repeated in the snapshot, leaves with no way
// to load anything.
(function () {
  'use strict';

  var data = JSON.parse(document.getElementById('trace-data').textContent);
  var actions = Array.isArray(data.actions) ? data.actions : [];
  var messages = Array.isArray(data.console) ? data.console : [];
  var requests = Array.isArray(data.network) ? data.network : [];
  var limits = data.limits && typeof data.limits === 'object' ? data.limits : {};
  var app = document.getElementById('app');
  var SNAPSHOT_POLICY = "default-src 'none'; style-src 'unsafe-inline'; img-src data:";
  var TABS = [
    ['screenshot', 'Screenshot'],
    ['snapshot', 'DOM snapshot'],
    ['details', 'Details'],
    ['console', 'Console'],
    ['network', 'Network']
  ];

  var selected = -1;
  var tab = 'screenshot';
  var rows = [];
  var bars = [];
  var tabButtons = {};
  var panel = null;

  function el(tag, className, text) {
    var node = document.createElement(tag);
    if (className) {
      node.className = className;
    }
    if (text !== undefined && text !== null) {
      node.textContent = String(text);
    }
    return node;
  }

  function empty(node) {
    while (node.firstChild) {
      node.removeChild(node.firstChild);
    }
  }

  function time(value) {
    var n = Number(value);
    if (!isFinite(n)) {
      return '';
    }
    return n < 1000 ? Math.round(n) + ' ms' : (n / 1000).toFixed(2) + ' s';
  }

  function total() {
    var end = Number(data.duration_ms) || 0;
    actions.forEach(function (action) {
      end = Math.max(end, Number(action.end_ms) || 0);
    });
    return end > 0 ? end : 1;
  }

  // ---- header and notes ----------------------------------------------------

  function header() {
    var box = el('header', 'top');
    box.appendChild(el('h1', '', 'Trace'));
    box.appendChild(
      el(
        'p',
        'meta',
        'Started ' + (data.started_at || '?') + ', ' + time(data.duration_ms) + ', ' +
          actions.length + ' actions, ' + messages.length + ' console messages, ' +
          requests.length + ' requests'
      )
    );
    var notes = [];
    if (limits.actions_skipped) {
      notes.push(limits.actions_skipped + ' actions after the limit were not recorded.');
    }
    if (limits.pictures_skipped) {
      notes.push(
        limits.pictures_skipped + ' actions have no screenshot or snapshot: the size limit was reached.'
      );
    }
    if (limits.console_dropped) {
      notes.push(limits.console_dropped + ' console messages were left out.');
    }
    if (limits.requests_dropped) {
      notes.push(limits.requests_dropped + ' requests were left out of the list.');
    }
    if (limits.capture_failures) {
      notes.push(limits.capture_failures + ' captures failed (a page navigating, for instance).');
    }
    if (limits.network_error) {
      notes.push('No network log: ' + limits.network_error);
    }
    notes.forEach(function (note) {
      box.appendChild(el('p', 'note', note));
    });
    return box;
  }

  // ---- the timeline and the action list ------------------------------------

  function timeline() {
    var span = total();
    var strip = el('div', 'timeline');
    actions.forEach(function (action, index) {
      var start = Number(action.start_ms) || 0;
      var end = Math.max(Number(action.end_ms) || start, start);
      var bar = el('button', 'bar ' + (action.ok ? 'ok' : 'fail'));
      bar.type = 'button';
      bar.title = (index + 1) + '. ' + action.name;
      bar.style.left = (start / span) * 100 + '%';
      bar.style.width = Math.max(((end - start) / span) * 100, 0.6) + '%';
      bar.addEventListener('click', function () {
        select(index);
      });
      bars.push(bar);
      strip.appendChild(bar);
    });
    return strip;
  }

  function actionList() {
    var list = el('ol', 'actions');
    actions.forEach(function (action, index) {
      var item = el('li', '');
      var button = el('button', 'action ' + (action.ok ? 'ok' : 'fail'));
      button.type = 'button';
      button.appendChild(el('span', 'num', index + 1));
      button.appendChild(el('span', 'name', action.name));
      if (action.selector) {
        button.appendChild(el('span', 'selector', action.selector));
      }
      button.appendChild(el('span', 'dur', time(Number(action.end_ms) - Number(action.start_ms))));
      button.addEventListener('click', function () {
        select(index);
      });
      item.appendChild(button);
      rows.push(item);
      list.appendChild(item);
    });
    if (actions.length === 0) {
      list.appendChild(el('li', 'none', 'No actions were recorded.'));
    }
    return list;
  }

  // ---- panels ---------------------------------------------------------------

  function current() {
    return selected >= 0 ? actions[selected] : null;
  }

  function nothing(text) {
    return el('p', 'none', text);
  }

  function screenshotPanel(action) {
    if (!action) {
      return nothing('Pick an action.');
    }
    if (!action.screenshot) {
      return nothing('This action has no screenshot.');
    }
    var image = el('img', 'shot');
    image.alt = 'The page after ' + action.name;
    image.src = 'data:image/jpeg;base64,' + action.screenshot;
    return image;
  }

  function snapshotPanel(action) {
    if (!action) {
      return nothing('Pick an action.');
    }
    if (typeof action.snapshot !== 'string' || action.snapshot === '') {
      return nothing('This action has no DOM snapshot.');
    }
    var box = el('div', 'snap');
    box.appendChild(el('p', 'meta', (action.title || '') + '  ' + (action.url || '')));
    var frame = el('iframe', 'snapframe');
    // The sandbox is set before the document is, and stays empty on purpose.
    frame.setAttribute('sandbox', '');
    frame.setAttribute('referrerpolicy', 'no-referrer');
    frame.title = 'DOM snapshot after ' + action.name;
    frame.srcdoc =
      '<meta http-equiv="Content-Security-Policy" content="' + SNAPSHOT_POLICY + '">' +
      action.snapshot;
    box.appendChild(frame);
    return box;
  }

  function detailRow(list, name, value) {
    if (value === undefined || value === null || value === '') {
      return;
    }
    list.appendChild(el('dt', '', name));
    list.appendChild(el('dd', '', value));
  }

  function detailsPanel(action) {
    if (!action) {
      return nothing('Pick an action.');
    }
    var list = el('dl', 'details');
    detailRow(list, 'Action', action.name);
    detailRow(list, 'Selector', action.selector);
    detailRow(list, 'Detail', action.detail);
    detailRow(list, 'Started', time(action.start_ms) + ' after the trace began');
    detailRow(list, 'Took', time(Number(action.end_ms) - Number(action.start_ms)));
    detailRow(list, 'Result', action.ok ? 'ok' : 'failed');
    detailRow(list, 'Error', action.error);
    detailRow(list, 'Page', action.url);
    detailRow(list, 'Title', action.title);
    return list;
  }

  function consolePanel(action) {
    if (messages.length === 0) {
      return nothing('The page logged nothing.');
    }
    var list = el('ul', 'console');
    var first = null;
    messages.forEach(function (message) {
      var mine = action && message.action === action.id;
      var item = el('li', 'msg ' + String(message.level || 'log') + (mine ? ' current' : ''));
      item.appendChild(el('span', 'at', time(message.at_ms)));
      item.appendChild(el('span', 'level', message.level));
      item.appendChild(el('span', 'text', message.text));
      if (mine && !first) {
        first = item;
      }
      list.appendChild(item);
    });
    if (first) {
      window.setTimeout(function () {
        first.scrollIntoView({ block: 'nearest' });
      }, 0);
    }
    return list;
  }

  function networkPanel(action) {
    if (requests.length === 0) {
      return nothing('No requests were recorded.');
    }
    var from = action ? Number(action.start_ms) : -1;
    var until = Infinity;
    if (action && selected + 1 < actions.length) {
      until = Number(actions[selected + 1].start_ms);
    }
    var table = el('table', 'network');
    var head = el('tr', '');
    ['At', 'Method', 'Status', 'URL', 'Took', 'Type'].forEach(function (name) {
      head.appendChild(el('th', '', name));
    });
    table.appendChild(head);
    requests.forEach(function (request) {
      var started = Number(request.started_ms) || 0;
      var mine = action && started >= from && started < until;
      var bad = Number(request.status) >= 400 || Number(request.status) === 0;
      var row = el('tr', (mine ? 'current ' : '') + (bad ? 'bad' : ''));
      row.appendChild(el('td', '', time(started)));
      row.appendChild(el('td', '', request.method));
      row.appendChild(el('td', '', request.status));
      row.appendChild(el('td', 'url', request.url));
      row.appendChild(el('td', '', time(request.time_ms)));
      row.appendChild(el('td', '', request.type));
      table.appendChild(row);
    });
    return table;
  }

  var PANELS = {
    screenshot: screenshotPanel,
    snapshot: snapshotPanel,
    details: detailsPanel,
    console: consolePanel,
    network: networkPanel
  };

  function render() {
    empty(panel);
    panel.appendChild(PANELS[tab](current()));
    TABS.forEach(function (entry) {
      tabButtons[entry[0]].className = 'tab' + (entry[0] === tab ? ' on' : '');
    });
  }

  function select(index) {
    if (index < 0 || index >= actions.length) {
      return;
    }
    selected = index;
    rows.forEach(function (row, at) {
      row.className = at === index ? 'on' : '';
    });
    bars.forEach(function (bar, at) {
      bar.className = 'bar ' + (actions[at].ok ? 'ok' : 'fail') + (at === index ? ' on' : '');
    });
    if (rows[index]) {
      rows[index].scrollIntoView({ block: 'nearest' });
    }
    render();
  }

  function tabs() {
    var bar = el('div', 'tabs');
    TABS.forEach(function (entry) {
      var button = el('button', 'tab', entry[1]);
      button.type = 'button';
      button.addEventListener('click', function () {
        tab = entry[0];
        render();
      });
      tabButtons[entry[0]] = button;
      bar.appendChild(button);
    });
    return bar;
  }

  // ---- the page -------------------------------------------------------------

  app.appendChild(header());
  app.appendChild(timeline());
  var main = el('main', 'main');
  var left = el('nav', 'left');
  left.appendChild(actionList());
  var right = el('section', 'right');
  right.appendChild(tabs());
  panel = el('div', 'panel');
  right.appendChild(panel);
  main.appendChild(left);
  main.appendChild(right);
  app.appendChild(main);

  document.addEventListener('keydown', function (event) {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      select(selected + (event.key === 'ArrowDown' ? 1 : -1));
    }
  });

  if (actions.length > 0) {
    select(0);
  } else {
    render();
  }
})();
