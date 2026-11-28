const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (sel) => document.querySelector(sel);

const state = {
  workspace: "",
  snapshot: null,
  selectedTargets: new Set(),
  selectedScenarios: new Set(),
  targetStatuses: new Map(),
  activeRunId: null,
  runQueue: [],
  activeQueueItem: null,
  activeCasePlan: null,
};

function showError(msg) {
  const banner = $("#status-banner");
  if (!msg) {
    banner.hidden = true;
    banner.textContent = "";
    return;
  }
  banner.textContent = msg;
  banner.hidden = false;
}

function setRunStatus(text, kind = "") {
  const pill = $("#global-run-status");
  pill.textContent = text;
  pill.className = kind ? `run-pill ${kind}` : "run-pill";
}

// ---------------- Accessible Tabs ----------------

const tabs = [...document.querySelectorAll('[role="tab"]')];
const panels = [...document.querySelectorAll('[role="tabpanel"]')];

function selectTab(nextTab, focusTab = false) {
  for (const tab of tabs) {
    const active = tab === nextTab;
    tab.ariaSelected = active ? "true" : "false";
    tab.toggleAttribute("focusgroupstart", active);
    const panel = document.getElementById(tab.dataset.panel);
    if (panel) {
      if (active) {
        panel.removeAttribute("hidden");
      } else {
        panel.setAttribute("hidden", "until-found");
      }
    }
  }
  if (focusTab) nextTab.focus();
}

for (const tab of tabs) {
  tab.addEventListener("click", () => selectTab(tab));
  tab.addEventListener("keydown", (event) => {
    const idx = tabs.indexOf(tab);
    let targetIdx = null;
    if (event.key === "ArrowRight") targetIdx = (idx + 1) % tabs.length;
    else if (event.key === "ArrowLeft") targetIdx = (idx - 1 + tabs.length) % tabs.length;
    else if (event.key === "Home") targetIdx = 0;
    else if (event.key === "End") targetIdx = tabs.length - 1;
    if (targetIdx !== null) {
      event.preventDefault();
      selectTab(tabs[targetIdx], true);
    }
  });
}

for (const panel of panels) {
  panel.addEventListener("beforematch", () => {
    const tab = tabs.find((t) => t.dataset.panel === panel.id);
    if (tab) selectTab(tab);
  });
}

// ---------------- Workspace Loading ----------------

async function loadWorkspace(path) {
  showError("");
  try {
    const snapshot = await invoke("inspect_workspace", { path });
    state.workspace = snapshot.root;
    state.snapshot = snapshot;
    $("#workspace-path").value = snapshot.root;

    const pkg = snapshot.package_name
      ? `${snapshot.package_name}${snapshot.package_version ? ` v${snapshot.package_version}` : ""}`
      : snapshot.root;
    const cli = snapshot.sbrs_binary ? ` · sbrs: ${snapshot.sbrs_binary}` : " · cargo fallback";
    $("#project-summary").textContent = `${pkg} · ${snapshot.targets.length} targets · ${snapshot.features.length} feature files${cli}`;

    renderTargets();
    renderBehave();
    renderCasePlans();
    renderLogs();
  } catch (err) {
    showError(String(err));
  }
}

// ---------------- Commander Targets ----------------

function targetKey(target) {
  return `${target.kind}:${target.name}`;
}

function visibleTargets() {
  if (!state.snapshot) return [];
  const query = $("#test-filter").value.trim().toLowerCase();
  const kindFilter = $("#kind-filter").value;
  return state.snapshot.targets.filter((target) => {
    if (kindFilter && target.kind !== kindFilter) return false;
    if (!query) return true;
    if (target.name.toLowerCase().includes(query) || target.path.toLowerCase().includes(query)) {
      return true;
    }
    return target.cases.some((c) => c.name.toLowerCase().includes(query));
  });
}

function renderTargets() {
  const list = $("#target-list");
  list.textContent = "";
  const items = visibleTargets();
  $("#targets-empty").hidden = items.length > 0;

  for (const target of items) {
    const key = targetKey(target);
    const li = document.createElement("li");
    li.className = "target-card";

    const row = document.createElement("div");
    row.className = "target-row";

    const label = document.createElement("label");
    label.className = "target-label";
    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    checkbox.checked = state.selectedTargets.has(key);
    checkbox.addEventListener("change", () => {
      if (checkbox.checked) state.selectedTargets.add(key);
      else state.selectedTargets.delete(key);
    });
    label.append(checkbox, document.createTextNode(target.name));

    const meta = document.createElement("div");
    meta.className = "target-meta";

    const status = state.targetStatuses.get(key);
    if (status) {
      const s = document.createElement("span");
      s.className = `status-badge ${status}`;
      s.textContent = status.toUpperCase();
      meta.append(s);
    }

    const kindBadge = document.createElement("span");
    kindBadge.className = "kind-badge";
    kindBadge.textContent =
      target.kind === "example" ? "example" : target.kind === "unit_suite" ? "lib" : "test";

    const runOne = document.createElement("button");
    runOne.type = "button";
    runOne.className = "btn-secondary";
    runOne.textContent = "Run";
    runOne.addEventListener("click", () => {
      enqueueRuns([{ target, caseFilter: $("#opt-case-filter").value.trim() || null }]);
    });

    meta.append(kindBadge, runOne);
    row.append(label, meta);
    li.append(row);

    if (target.cases.length > 0) {
      const sublist = document.createElement("div");
      sublist.className = "case-sublist";
      for (const tc of target.cases) {
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = "case-btn font-mono";
        btn.textContent = tc.ignored ? `${tc.name} [ignored]` : tc.name;
        btn.addEventListener("click", () => {
          enqueueRuns([
            {
              target,
              caseFilter: tc.name,
              forceIgnored: tc.ignored,
            },
          ]);
        });
        sublist.append(btn);
      }
      li.append(sublist);
    }

    list.append(li);
  }
}

function collectExtraEnv() {
  const raw = $("#opt-extra-env").value.trim();
  if (!raw) return null;
  const out = {};
  for (const part of raw.split(/\s+/)) {
    const eq = part.indexOf("=");
    if (eq > 0) {
      out[part.slice(0, eq)] = part.slice(eq + 1);
    }
  }
  return Object.keys(out).length ? out : null;
}

function baseRunOptions() {
  return {
    workspace: state.workspace,
    features: $("#opt-features").value.trim() || null,
    include_ignored: $("#flag-ignored").checked,
    browser: $("#opt-browser").value || null,
    headless: $("#flag-headless").checked,
    demo: $("#flag-demo").checked,
    slow: $("#flag-slow").checked,
    uc: $("#flag-uc").checked,
    reuse_session: $("#flag-reuse").checked,
    incognito: $("#flag-incognito").checked,
    ad_block: $("#flag-adblock").checked,
    extra_env: collectExtraEnv(),
  };
}

function enqueueRuns(jobs) {
  if (!jobs.length) {
    showError("Select at least one test or example to run.");
    return;
  }
  showError("");
  state.runQueue = jobs.map((job) => ({
    key: job.target ? targetKey(job.target) : null,
    request: {
      ...baseRunOptions(),
      mode: job.cliArgs ? "sbrs_cli" : "cargo_target",
      target: job.target ? job.target.name : null,
      target_kind: job.target ? job.target.kind : null,
      filter: job.caseFilter ?? null,
      include_ignored: Boolean(job.forceIgnored) || $("#flag-ignored").checked,
      features: job.features ?? ($("#opt-features").value.trim() || null),
      cli_args: job.cliArgs ?? null,
    },
  }));
  runNextInQueue();
}

async function runNextInQueue() {
  if (state.activeRunId !== null) return;
  const next = state.runQueue.shift();
  if (!next) {
    $("#stop-run-btn").disabled = true;
    $("#run-selected-btn").disabled = false;
    return;
  }
  state.activeQueueItem = next;
  if (next.key) {
    state.targetStatuses.set(next.key, "running");
    renderTargets();
  }
  setRunStatus("Running...", "running");
  $("#stop-run-btn").disabled = false;
  $("#run-selected-btn").disabled = true;

  try {
    state.activeRunId = await invoke("start_run", { request: next.request });
  } catch (err) {
    showError(String(err));
    state.activeRunId = null;
    state.runQueue = [];
    setRunStatus("Error", "fail");
    $("#stop-run-btn").disabled = true;
    $("#run-selected-btn").disabled = false;
  }
}

function appendConsole(line, stream) {
  const box = $("#console-output");
  const span = document.createElement("span");
  if (stream === "stderr") span.className = "log-stderr";
  else if (stream === "meta") span.className = "log-meta";
  span.textContent = `${line}\n`;
  box.append(span);
  box.scrollTop = box.scrollHeight;
}

// ---------------- Behave ----------------

function scenarioKey(featurePath, scenario) {
  return `${featurePath}::${scenario.filter}`;
}

function parseTagExpr(expr) {
  const trimmed = expr.trim();
  if (!trimmed) return { include: [], exclude: [] };
  const include = [];
  const exclude = [];
  for (const part of trimmed.split(/[,\s]+/)) {
    if (!part) continue;
    if (part.startsWith("-") || part.startsWith("~")) {
      exclude.push(part.slice(1).replace(/^@/, "").toLowerCase());
    } else {
      include.push(part.replace(/^@/, "").toLowerCase());
    }
  }
  return { include, exclude };
}

function scenarioMatchesTags(scenario, expr) {
  const tags = scenario.tags.map((t) => t.toLowerCase());
  if (expr.exclude.some((t) => tags.includes(t))) return false;
  if (expr.include.length && !expr.include.some((t) => tags.includes(t))) return false;
  return true;
}

function renderBehave() {
  const list = $("#behave-list");
  list.textContent = "";
  const features = state.snapshot?.features ?? [];
  $("#behave-empty").hidden = features.length > 0;
  const tagExpr = parseTagExpr($("#behave-tag-filter").value);

  for (const feat of features) {
    const li = document.createElement("li");
    li.className = "target-card";
    const title = document.createElement("div");
    title.className = "target-row";
    const strong = document.createElement("strong");
    strong.textContent = `${feat.path} -> ${feat.name || "(untitled feature)"}`;
    title.append(strong);
    li.append(title);

    for (const sc of feat.scenarios) {
      if (!scenarioMatchesTags(sc, tagExpr)) continue;
      const key = scenarioKey(feat.path, sc);
      const label = document.createElement("label");
      label.className = "check-chip";
      const cb = document.createElement("input");
      cb.type = "checkbox";
      cb.checked = state.selectedScenarios.has(key);
      cb.addEventListener("change", () => {
        if (cb.checked) state.selectedScenarios.add(key);
        else state.selectedScenarios.delete(key);
      });
      const tagSuffix = sc.tags.length ? ` [@${sc.tags.join(" @")}]` : "";
      label.append(
        cb,
        document.createTextNode(`line ${sc.line}: ${sc.name} -> ${sc.filter}${tagSuffix}`),
      );
      li.append(label);
    }
    list.append(li);
  }
}

// ---------------- Case Plans ----------------

function renderCasePlans() {
  const list = $("#caseplan-list");
  list.textContent = "";
  const plans = state.snapshot?.case_plans ?? [];
  $("#caseplans-empty").hidden = plans.length > 0;

  for (const plan of plans) {
    const li = document.createElement("li");
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className =
      state.activeCasePlan?.plan_path === plan.plan_path
        ? "list-item-btn selected"
        : "list-item-btn";
    const name = document.createElement("span");
    name.className = "font-mono";
    name.textContent = plan.id;
    const badge = document.createElement("span");
    badge.className = `status-badge ${plan.exists ? "pass" : ""}`;
    badge.textContent = plan.exists ? "saved" : "new";
    btn.append(name, badge);
    btn.addEventListener("click", () => openCasePlan(plan));
    li.append(btn);
    list.append(li);
  }
}

async function openCasePlan(plan) {
  showError("");
  try {
    const content = await invoke("read_case_plan", {
      workspace: state.workspace,
      planPath: plan.plan_path,
    });
    state.activeCasePlan = plan;
    $("#active-plan-path").textContent = plan.plan_path;
    $("#caseplan-editor").value = content;
    $("#save-caseplan-btn").disabled = false;
    renderCasePlans();
  } catch (err) {
    showError(String(err));
  }
}

// ---------------- Logs ----------------

function renderLogs() {
  const list = $("#logs-list");
  list.textContent = "";
  const logs = state.snapshot?.logs ?? [];
  $("#logs-empty").hidden = logs.length > 0;

  for (const log of logs) {
    const li = document.createElement("li");
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "list-item-btn font-mono";
    btn.textContent = log.name;
    btn.addEventListener("click", () => previewLogFolder(log));
    li.append(btn);
    list.append(li);
  }
}

async function previewLogFolder(log) {
  const preview = $("#log-preview");
  const img = $("#log-screenshot");
  const info = $("#log-info-text");
  img.hidden = true;
  info.hidden = true;
  preview.hidden = false;

  if (log.screenshot_path) {
    const res = await invoke("read_log_artifact", {
      workspace: state.workspace,
      artifactPath: log.screenshot_path,
    });
    if (res.image_data_url) {
      img.src = res.image_data_url;
      img.hidden = false;
    }
  }
  if (log.info_path) {
    const res = await invoke("read_log_artifact", {
      workspace: state.workspace,
      artifactPath: log.info_path,
    });
    if (res.text) {
      info.textContent = res.text;
      info.hidden = false;
    }
  }
}

// ---------------- Event Wiring ----------------

$("#workspace-form").addEventListener("submit", (e) => {
  e.preventDefault();
  loadWorkspace($("#workspace-path").value.trim());
});

$("#test-filter").addEventListener("input", renderTargets);
$("#kind-filter").addEventListener("change", renderTargets);
$("#behave-tag-filter").addEventListener("input", renderBehave);

$("#select-visible-tests").addEventListener("click", () => {
  for (const t of visibleTargets()) state.selectedTargets.add(targetKey(t));
  renderTargets();
});

$("#clear-selected-tests").addEventListener("click", () => {
  state.selectedTargets.clear();
  renderTargets();
});

$("#run-selected-btn").addEventListener("click", () => {
  const selected = visibleTargets().filter((t) => state.selectedTargets.has(targetKey(t)));
  const caseFilter = $("#opt-case-filter").value.trim() || null;
  enqueueRuns(selected.map((target) => ({ target, caseFilter })));
});

$("#stop-run-btn").addEventListener("click", async () => {
  state.runQueue = [];
  await invoke("stop_run");
});

$("#clear-console-btn").addEventListener("click", () => {
  $("#console-output").textContent = "";
});

$("#run-behave-btn").addEventListener("click", () => {
  const filters = [...state.selectedScenarios].map((k) => k.split("::")[1]).filter(Boolean);
  if (!filters.length) {
    showError("Check at least one scenario to run.");
    return;
  }
  const integrationTargets = (state.snapshot?.targets ?? []).filter(
    (t) => t.kind === "integration_test",
  );
  const bddFeat = $("#behave-features").value.trim() || "bdd,testing";
  const jobs = [];
  for (const target of integrationTargets) {
    for (const filter of filters) {
      jobs.push({ target, caseFilter: filter, features: bddFeat });
    }
  }
  enqueueRuns(jobs);
});

$("#save-caseplan-btn").addEventListener("click", async () => {
  if (!state.activeCasePlan) return;
  try {
    await invoke("write_case_plan", {
      workspace: state.workspace,
      planPath: state.activeCasePlan.plan_path,
      content: $("#caseplan-editor").value,
    });
    state.activeCasePlan.exists = true;
    renderCasePlans();
    setRunStatus("Saved plan", "pass");
  } catch (err) {
    showError(String(err));
  }
});

$("#mkrec-form").addEventListener("submit", (e) => {
  e.preventDefault();
  const file = $("#mkrec-file").value.trim();
  if (!file) return;
  const cliArgs = ["mkrec", file];
  if ($("#mkrec-overwrite").checked) cliArgs.push("--overwrite");
  if ($("#mkrec-behave").checked) cliArgs.push("--behave");
  enqueueRuns([{ cliArgs }]);
});

$("#launch-recorder-btn").addEventListener("click", () => {
  enqueueRuns([{ cliArgs: ["recorder"] }]);
});

$("#list-plugins-btn").addEventListener("click", () => {
  enqueueRuns([{ cliArgs: ["plugin", "list"] }]);
});

const dialog = $("#new-feature-dialog");
$("#open-new-feature-btn").addEventListener("click", () => dialog.showModal());
$("#cancel-new-feature").addEventListener("click", () => dialog.close());
$("#new-feature-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  try {
    await invoke("create_feature_file", {
      workspace: state.workspace,
      name: $("#nf-name").value,
      title: $("#nf-title").value,
      scenarios: $("#nf-scenarios").value.split("\n"),
    });
    dialog.close();
    await loadWorkspace(state.workspace);
  } catch (err) {
    showError(String(err));
  }
});

async function init() {
  await listen("runner-log", ({ payload }) => {
    appendConsole(payload.line, payload.stream);
  });

  await listen("runner-finished", ({ payload }) => {
    if (state.activeQueueItem?.key) {
      state.targetStatuses.set(state.activeQueueItem.key, payload.success ? "pass" : "fail");
      renderTargets();
    }
    const secs = (payload.duration_ms / 1000).toFixed(2);
    appendConsole(
      `--- Finished in ${secs}s (exit ${payload.exit_code ?? "signal"}${payload.stopped ? ", stopped" : ""}) ---`,
      "meta",
    );
    state.activeRunId = null;
    if (payload.stopped) {
      setRunStatus("Stopped", "fail");
      state.runQueue = [];
    } else if (payload.success) {
      setRunStatus(`Passed (${secs}s)`, "pass");
    } else {
      setRunStatus(`Failed (${secs}s)`, "fail");
    }
    runNextInQueue();
    loadWorkspace(state.workspace);
  });

  const initialDir = await invoke("default_workspace");
  await loadWorkspace(initialDir);
}

init();
