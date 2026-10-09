const { invoke } = window.__TAURI__.core;

const $ = (sel) => document.querySelector(sel);
const statusEl = $("#status");

function setStatus(msg) {
  statusEl.textContent = msg;
}

function el(tag, { className, text, attrs } = {}) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text != null) node.textContent = text;
  if (attrs) {
    for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, value);
  }
  return node;
}

// ---------------- State ----------------

let profiles = [];
let tags = [];
let folders = [];
let selectedProfileIds = new Set();
// The fingerprint of the profile open in the dialog. The form shows only some of
// its fields; the rest are kept when the profile is saved.
let editingFingerprint;
// Set when the profile store could not be opened: an empty list then means
// "locked", not "no profiles".
let storageLocked = false;

async function apiBase() {
  return invoke("get_api_base");
}

// The local API is reachable by anything running on this machine, including
// any web page the user has open, so it requires a token that only this
// window can obtain over Tauri IPC. Fetched once and reused.
let apiTokenPromise = null;

async function apiToken() {
  if (!apiTokenPromise) {
    apiTokenPromise = invoke("get_api_token").catch((err) => {
      // Let a later call retry rather than caching the failure forever.
      apiTokenPromise = null;
      throw err;
    });
  }
  return apiTokenPromise;
}

async function api(path, options = {}) {
  const [base, token] = await Promise.all([apiBase(), apiToken()]);
  // Build the headers explicitly: spreading `options` last would let a
  // caller-supplied `headers` object replace the defaults wholesale, and the
  // Authorization header is set after the merge so nothing can override it.
  const headers = new Headers({ "content-type": "application/json" });
  for (const [key, value] of new Headers(options.headers || {})) {
    headers.set(key, value);
  }
  headers.set("Authorization", `Bearer ${token}`);
  const res = await fetch(`${base}${path}`, { ...options, headers });
  const body = await res.json().catch(() => ({}));
  if (!res.ok || (body.status && body.status.error_code)) {
    const msg = body.status?.message || `HTTP ${res.status}`;
    throw new Error(msg);
  }
  return body.data;
}

// ---------------- Rendering ----------------

function folderName(id) {
  const folder = folders.find((f) => f.id === id);
  return folder ? folder.name : "Default";
}

function tagById(id) {
  return tags.find((t) => t.id === id || t.name === id);
}

function passesFilters(p) {
  const query = $("#profile-search").value.trim().toLowerCase();
  const folderId = $("#folder-filter").value;
  const tagId = $("#tag-filter").value;
  if (query && !p.name.toLowerCase().includes(query) && !p.container_url.toLowerCase().includes(query)) {
    return false;
  }
  if (folderId && p.folder_id !== folderId) return false;
  if (tagId && !(p.tags || []).includes(tagId)) return false;
  return true;
}

function renderProfiles() {
  const list = $("#profile-list");
  list.textContent = "";
  const visible = profiles.filter(passesFilters);
  $("#profiles-empty").hidden = visible.length > 0 || storageLocked;

  for (const p of visible) {
    const card = el("li", { className: "profile-card" });

    const head = el("header");
    const checkbox = el("input", { className: "select-profile", attrs: { type: "checkbox" } });
    checkbox.checked = selectedProfileIds.has(p.id);
    checkbox.setAttribute("aria-label", `Select profile ${p.name}`);
    checkbox.addEventListener("change", () => {
      if (checkbox.checked) selectedProfileIds.add(p.id);
      else selectedProfileIds.delete(p.id);
    });

    const titleWrap = el("div");
    titleWrap.appendChild(el("strong", { text: p.name }));
    const where = p.engine === "PureCdp" ? "Chrome on this computer" : p.container_url;
    const meta = el("div", { className: "muted", text: `${where} · ${folderName(p.folder_id)}` });
    titleWrap.appendChild(meta);
    head.append(checkbox, titleWrap);
    card.appendChild(head);

    const badges = el("div", { className: "badges" });
    badges.appendChild(el("span", { className: "tag", text: p.browser || "Chrome" }));
    badges.appendChild(el("span", { className: "tag", text: p.engine === "PureCdp" ? "Pure CDP" : p.mode || "WebDriver" }));
    if (p.headless) badges.appendChild(el("span", { className: "tag", text: "headless" }));
    if (p.external_profile) badges.appendChild(el("span", { className: "tag external", text: "external" }));
    if (p.fingerprint) badges.appendChild(el("span", { className: "tag external", text: "masked" }));
    if (p.latitude != null && p.longitude != null) {
      badges.appendChild(el("span", { className: "tag", text: `${p.latitude.toFixed(2)}, ${p.longitude.toFixed(2)}` }));
    }
    for (const tagId of p.tags || []) {
      const tag = tagById(tagId);
      if (tag) badges.appendChild(el("span", { className: "tag", text: tag.name }));
    }
    card.appendChild(badges);

    const launchRow = el("div", { className: "actions" });
    const urlInput = el("input", { attrs: { type: "url", placeholder: "Start URL (optional)", "aria-label": `Start URL for ${p.name}` } });
    const launchBtn = el("button", { text: "Launch" });
    launchBtn.addEventListener("click", async () => {
      setStatus(`Launching ${p.name}…`);
      try {
        const info = await invoke("launch_profile", { id: p.id, startUrl: urlInput.value || undefined });
        setStatus(`Launched ${info.profile_name} → ${info.session_id}`);
        refreshSessions();
      } catch (e) {
        setStatus(`Launch failed: ${e}`);
      }
    });
    launchRow.append(urlInput, launchBtn);
    card.appendChild(launchRow);

    const manageRow = el("div", { className: "actions" });
    const editBtn = el("button", { className: "secondary", text: "Edit" });
    editBtn.addEventListener("click", () => openProfileDialog(p));
    const cloneBtn = el("button", { className: "secondary", text: "Clone" });
    cloneBtn.addEventListener("click", () => cloneProfile(p));
    const randomizeBtn = el("button", { className: "secondary", text: "Randomize" });
    randomizeBtn.setAttribute("aria-label", `Give ${p.name} a random identity`);
    randomizeBtn.addEventListener("click", () => randomizeProfile(p));
    const exportBtn = el("button", { className: "secondary", text: "Export" });
    exportBtn.addEventListener("click", () => exportProfile(p));
    const deleteBtn = el("button", { className: "danger", text: "Delete" });
    deleteBtn.addEventListener("click", () => confirmDeleteProfile(p));
    manageRow.append(editBtn, cloneBtn, randomizeBtn, exportBtn, deleteBtn);
    card.appendChild(manageRow);

    list.appendChild(card);
  }
}

async function refreshProfiles() {
  try {
    profiles = await invoke("list_profiles");
    renderProfiles();
  } catch (e) {
    setStatus(`Profile refresh failed: ${e}`);
  }
}

async function refreshSessions() {
  const list = $("#session-list");
  list.textContent = "";
  let sessions = [];
  try {
    sessions = await invoke("list_sessions");
  } catch (e) {
    setStatus(`Session refresh failed: ${e}`);
  }
  $("#sessions-empty").hidden = sessions.length > 0;

  for (const s of sessions) {
    const li = el("li");
    const head = el("div", { className: "actions" });
    head.append(el("strong", { text: s.profile_name }));
    head.append(el("span", { className: "muted", text: s.engine === "PureCdp" ? "Pure CDP" : s.container_url }));
    li.appendChild(head);

    const navRow = el("div", { className: "actions" });
    const navInput = el("input", { attrs: { type: "url", placeholder: "Navigate to URL", "aria-label": `Navigate session for ${s.profile_name}` } });
    const navBtn = el("button", { text: "Go" });
    navBtn.addEventListener("click", async () => {
      if (!navInput.value) return;
      setStatus("Navigating…");
      try {
        await invoke("navigate_session", { sessionId: s.session_id, url: navInput.value });
        setStatus("Navigation complete");
      } catch (e) {
        setStatus(`Navigation failed: ${e}`);
      }
    });
    const shotBtn = el("button", { className: "secondary", text: "Screenshot" });
    shotBtn.addEventListener("click", async () => {
      setStatus("Taking screenshot…");
      try {
        const path = await invoke("take_screenshot", { sessionId: s.session_id });
        setStatus(`Screenshot saved: ${path}`);
      } catch (e) {
        setStatus(`Screenshot failed: ${e}`);
      }
    });
    navRow.append(navInput, navBtn, shotBtn);
    li.appendChild(navRow);

    const geoRow = el("div", { className: "actions" });
    const lat = el("input", { attrs: { type: "number", step: "any", placeholder: "Lat", "aria-label": "Latitude" } });
    const lon = el("input", { attrs: { type: "number", step: "any", placeholder: "Lon", "aria-label": "Longitude" } });
    const acc = el("input", { attrs: { type: "number", step: "any", placeholder: "Accuracy m", "aria-label": "Accuracy in meters" } });
    const geoBtn = el("button", { className: "secondary", text: "Set geolocation" });
    geoBtn.addEventListener("click", async () => {
      const latNum = Number(lat.value);
      const lonNum = Number(lon.value);
      if (Number.isNaN(latNum) || Number.isNaN(lonNum)) {
        setStatus("Enter a valid latitude and longitude");
        return;
      }
      try {
        await invoke("set_session_geolocation", {
          sessionId: s.session_id,
          latitude: latNum,
          longitude: lonNum,
          accuracy: acc.value ? Number(acc.value) : undefined,
        });
        setStatus("Geolocation updated");
      } catch (e) {
        setStatus(`Geolocation failed: ${e}`);
      }
    });
    const closeBtn = el("button", { className: "danger", text: "Close session" });
    closeBtn.addEventListener("click", async () => {
      await invoke("close_session", { sessionId: s.session_id });
      refreshSessions();
    });
    geoRow.append(lat, lon, acc, geoBtn, closeBtn);
    li.appendChild(geoRow);

    list.appendChild(li);
  }
}

// ---------------- Folders & tags ----------------

function renderOrgLists() {
  const folderList = $("#folder-list");
  folderList.textContent = "";
  for (const f of folders) {
    const li = el("li");
    li.appendChild(el("span", { text: f.name }));
    const rename = el("button", { className: "chip-btn", text: "Rename", attrs: { "aria-label": `Rename folder ${f.name}` } });
    rename.addEventListener("click", async () => {
      const name = prompt(`Rename folder "${f.name}"`, f.name);
      if (!name || name === f.name) return;
      try {
        await api(`/api/v1/folders/${f.id}`, { method: "POST", body: JSON.stringify({ name }) });
        refreshOrg();
      } catch (e) {
        setStatus(`Rename failed: ${e}`);
      }
    });
    const del = el("button", { className: "chip-btn", text: "Delete", attrs: { "aria-label": `Delete folder ${f.name}` } });
    del.addEventListener("click", async () => {
      try {
        await api(`/api/v1/folders/${f.id}`, { method: "DELETE" });
        refreshOrg();
      } catch (e) {
        setStatus(`Delete failed: ${e}`);
      }
    });
    li.append(rename, del);
    folderList.appendChild(li);
  }

  const tagList = $("#tag-list");
  tagList.textContent = "";
  for (const t of tags) {
    const li = el("li");
    li.appendChild(el("span", { text: `${t.name}` }));
    const swatch = el("span", { attrs: { "aria-hidden": "true" } });
    swatch.style.cssText = `display:inline-block;width:0.8em;height:0.8em;border-radius:50%;background:${t.color}`;
    li.appendChild(swatch);
    const rename = el("button", { className: "chip-btn", text: "Rename", attrs: { "aria-label": `Rename tag ${t.name}` } });
    rename.addEventListener("click", async () => {
      const name = prompt(`Rename tag "${t.name}"`, t.name);
      if (!name || name === t.name) return;
      try {
        await api(`/api/v1/tags/${t.id}`, { method: "POST", body: JSON.stringify({ name }) });
        refreshOrg();
      } catch (e) {
        setStatus(`Rename failed: ${e}`);
      }
    });
    const del = el("button", { className: "chip-btn", text: "Delete", attrs: { "aria-label": `Delete tag ${t.name}` } });
    del.addEventListener("click", async () => {
      try {
        await api(`/api/v1/tags/${t.id}`, { method: "DELETE" });
        refreshOrg();
      } catch (e) {
        setStatus(`Delete failed: ${e}`);
      }
    });
    li.append(rename, del);
    tagList.appendChild(li);
  }

  const folderFilter = $("#folder-filter");
  folderFilter.textContent = "";
  folderFilter.appendChild(el("option", { text: "All folders", attrs: { value: "" } }));
  for (const f of folders) {
    folderFilter.appendChild(el("option", { text: f.name, attrs: { value: f.id } }));
  }

  const tagFilter = $("#tag-filter");
  tagFilter.textContent = "";
  tagFilter.appendChild(el("option", { text: "All tags", attrs: { value: "" } }));
  for (const t of tags) {
    tagFilter.appendChild(el("option", { text: t.name, attrs: { value: t.id } }));
  }
}

async function refreshOrg() {
  try {
    const [tagData, folderData] = await Promise.all([
      api("/api/v1/tags"),
      api("/api/v1/folders"),
    ]);
    tags = tagData || [];
    folders = folderData || [];
    renderOrgLists();
    renderProfiles();
  } catch (e) {
    setStatus(`Refresh failed: ${e}`);
  }
}

// ---------------- Profile dialog ----------------

const profileDialog = $("#profile-dialog");

function fillDialogTags(selected) {
  const wrap = $("#profile-tags");
  wrap.textContent = "";
  for (const t of tags) {
    const label = el("label");
    const box = el("input", { attrs: { type: "checkbox", value: t.id } });
    box.checked = selected.includes(t.id);
    label.append(box, document.createTextNode(t.name));
    wrap.appendChild(label);
  }
}

function fillDialogFolders(current) {
  const sel = $("#profile-folder");
  sel.textContent = "";
  sel.appendChild(el("option", { text: "Default", attrs: { value: "default" } }));
  for (const f of folders) {
    sel.appendChild(el("option", { text: f.name, attrs: { value: f.id } }));
  }
  sel.value = current || "default";
}

function openProfileDialog(profile) {
  const isEdit = Boolean(profile);
  $("#profile-dialog-title").textContent = isEdit ? `Edit ${profile.name}` : "New profile";
  $("#profile-id").value = isEdit ? profile.id : "";
  $("#profile-name").value = isEdit ? profile.name : "";
  $("#profile-engine").value = isEdit ? profile.engine || "WebDriver" : "WebDriver";
  $("#container-url").value = isEdit ? profile.container_url : "";
  syncEngineFields();
  $("#profile-browser").value = isEdit ? (profile.browser || "Chrome") : "Chrome";
  $("#profile-mode").value = isEdit ? (profile.mode || "WebDriver") : "WebDriver";
  $("#user-agent").value = isEdit ? (profile.user_agent || "") : "";
  $("#proxy").value = isEdit ? (profile.proxy || "") : "";
  $("#locale").value = isEdit ? (profile.locale || "") : "";
  $("#latitude").value = isEdit && profile.latitude != null ? profile.latitude : "";
  $("#longitude").value = isEdit && profile.longitude != null ? profile.longitude : "";
  $("#accuracy").value = isEdit && profile.accuracy != null ? profile.accuracy : "";
  $("#headless").checked = isEdit ? Boolean(profile.headless) : false;
  fillDialogFolders(isEdit ? profile.folder_id : "default");
  fillDialogTags(isEdit ? profile.tags || [] : []);
  editingFingerprint = isEdit ? profile.fingerprint : undefined;
  fillFingerprint(editingFingerprint);
  profileDialog.showModal();
}

// A Pure CDP profile starts Chrome itself, so the fields that describe a
// WebDriver server do not apply to it.
function syncEngineFields() {
  const webdriver = $("#profile-engine").value === "WebDriver";
  $("#container-url").required = webdriver;
  $("#container-url").disabled = !webdriver;
  $("#profile-browser").disabled = !webdriver;
  $("#profile-mode").disabled = !webdriver;
}

async function submitProfileDialog(e) {
  e.preventDefault();
  const parseNum = (id) => {
    const v = $(id).value.trim();
    return v ? Number(v) : undefined;
  };
  const selectedTags = [...document.querySelectorAll("#profile-tags input:checked")].map((i) => i.value);
  const id = $("#profile-id").value;
  const body = {
    name: $("#profile-name").value.trim(),
    engine: $("#profile-engine").value,
    container_url: $("#profile-engine").value === "WebDriver" ? $("#container-url").value.trim() : "",
    browser: $("#profile-browser").value,
    mode: $("#profile-mode").value,
    user_agent: $("#user-agent").value || undefined,
    proxy: $("#proxy").value || undefined,
    locale: $("#locale").value || undefined,
    latitude: parseNum("#latitude"),
    longitude: parseNum("#longitude"),
    accuracy: parseNum("#accuracy"),
    headless: $("#headless").checked,
    folder_id: $("#profile-folder").value,
    tags: selectedTags,
  };
  const fingerprint = buildFingerprint(editingFingerprint);
  if (fingerprint) body.fingerprint = fingerprint;
  else if (id) body.fingerprint = null; // explicit clear on edit

  try {
    if (id) {
      await api(`/api/v1/profiles/${id}`, { method: "POST", body: JSON.stringify(body) });
      setStatus(`Updated ${body.name}`);
    } else {
      await invoke("create_profile", {
        new: {
          name: body.name,
          engine: body.engine,
          containerUrl: body.container_url,
          browser: body.browser,
          mode: body.mode,
          userAgent: body.user_agent,
          proxy: body.proxy,
          locale: body.locale,
          latitude: body.latitude,
          longitude: body.longitude,
          accuracy: body.accuracy,
          headless: body.headless,
          folderId: body.folder_id,
          tags: body.tags,
          fingerprint: body.fingerprint,
        },
      });
      setStatus(`Created ${body.name}`);
    }
    profileDialog.close();
    refreshProfiles();
  } catch (e) {
    setStatus(`Save failed: ${e}`);
  }
}

// ---------------- Confirm dialog ----------------

const confirmDialog = $("#confirm-dialog");
let confirmAction = null;

function askConfirm(message, action, okLabel = "Delete") {
  $("#confirm-message").textContent = message;
  $("#confirm-ok").textContent = okLabel;
  confirmAction = action;
  confirmDialog.showModal();
}

function confirmDeleteProfile(p) {
  askConfirm(`Delete profile "${p.name}"? This cannot be undone.`, async () => {
    try {
      await invoke("delete_profile", { id: p.id });
      setStatus(`Deleted ${p.name}`);
      refreshProfiles();
    } catch (e) {
      setStatus(`Delete failed: ${e}`);
    }
  });
}

// ---------------- Profile actions ----------------

async function cloneProfile(p) {
  setStatus(`Cloning ${p.name}…`);
  try {
    const clone = await api(`/api/v1/profiles/${p.id}/clone`, { method: "POST" });
    const newName = `${p.name} (copy)`;
    await api(`/api/v1/profiles/${clone.id}`, { method: "POST", body: JSON.stringify({ name: newName }) });
    setStatus(`Cloned as ${newName}`);
    refreshProfiles();
  } catch (e) {
    setStatus(`Clone failed: ${e}`);
  }
}

async function randomizeProfile(p) {
  const run = async () => {
    setStatus(`Randomizing ${p.name}…`);
    try {
      const result = await invoke("randomize_fingerprint", { id: p.id });
      setStatus(`Randomized ${p.name}: ${result.os} identity, seed ${result.seed}`);
      refreshProfiles();
    } catch (e) {
      setStatus(`Randomize failed: ${e}`);
    }
  };
  if (p.fingerprint) {
    askConfirm(`Replace the masking settings of "${p.name}" with a random identity?`, run, "Randomize");
  } else {
    await run();
  }
}

async function exportProfile(p) {
  try {
    const data = await api(`/api/v1/profiles/${p.id}/export`);
    const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
    const a = el("a", { attrs: { download: `${p.name}-profile.json` } });
    a.href = URL.createObjectURL(blob);
    a.click();
    URL.revokeObjectURL(a.href);
    setStatus(`Exported ${p.name}`);
  } catch (e) {
    setStatus(`Export failed: ${e}`);
  }
}

// ---------------- Wire up ----------------

window.addEventListener("DOMContentLoaded", async () => {
  fillFlagSelects();
  $("#open-create").addEventListener("click", () => openProfileDialog(null));
  $("#profile-form").addEventListener("submit", submitProfileDialog);
  $("#cancel-profile").addEventListener("click", () => profileDialog.close());
  $("#profile-engine").addEventListener("change", syncEngineFields);

  $("#confirm-ok").addEventListener("click", async () => {
    confirmDialog.close();
    if (confirmAction) await confirmAction();
    confirmAction = null;
  });
  $("#confirm-cancel").addEventListener("click", () => {
    confirmDialog.close();
    confirmAction = null;
  });

  $("#profile-search").addEventListener("input", renderProfiles);
  $("#folder-filter").addEventListener("change", renderProfiles);
  $("#tag-filter").addEventListener("change", renderProfiles);
  $("#refresh-profiles").addEventListener("click", refreshProfiles);
  $("#refresh-sessions").addEventListener("click", refreshSessions);

  $("#folder-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    const name = $("#new-folder").value.trim();
    if (!name) return;
    try {
      await api("/api/v1/folders", { method: "POST", body: JSON.stringify({ name }) });
      $("#new-folder").value = "";
      refreshOrg();
    } catch (e) {
      setStatus(`Add folder failed: ${e}`);
    }
  });

  $("#tag-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    const name = $("#new-tag").value.trim();
    if (!name) return;
    try {
      await api("/api/v1/tags", {
        method: "POST",
        body: JSON.stringify({ name, color: $("#new-tag-color").value }),
      });
      $("#new-tag").value = "";
      refreshOrg();
    } catch (e) {
      setStatus(`Add tag failed: ${e}`);
    }
  });

  $("#run-script").addEventListener("click", async () => {
    const script = $("#run-script-body").value.trim();
    if (!script || selectedProfileIds.size === 0) {
      setStatus("Enter a script and select at least one profile");
      return;
    }
    setStatus("Running script…");
    try {
      const data = await api("/api/v1/script_runner/start", {
        method: "POST",
        body: JSON.stringify({ profile_ids: [...selectedProfileIds], script }),
      });
      $("#run-script-result").textContent = JSON.stringify(data, null, 2);
      setStatus("Script run complete");
    } catch (e) {
      setStatus(`Script run failed: ${e}`);
    }
  });

  $("#import-profile").addEventListener("click", async () => {
    const raw = $("#import-profile-json").value.trim();
    if (!raw) {
      setStatus("Paste profile JSON to import");
      return;
    }
    try {
      const data = await api("/api/v1/profiles/import", { method: "POST", body: raw });
      setStatus(`Imported ${data?.name || "profile"}`);
      $("#import-profile-json").value = "";
      refreshProfiles();
    } catch (e) {
      setStatus(`Import failed: ${e}`);
    }
  });

  $("#proxy-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    const payload = {
      type: $("#proxy-type").value,
      host: $("#proxy-host").value.trim(),
      port: Number($("#proxy-port").value),
      username: $("#proxy-username").value || undefined,
      password: $("#proxy-password").value || undefined,
    };
    setStatus("Validating proxy…");
    try {
      const data = await api("/api/v1/proxy/validate", { method: "POST", body: JSON.stringify(payload) });
      $("#proxy-result").textContent = JSON.stringify(data, null, 2);
      setStatus("Proxy validation complete");
    } catch (e) {
      setStatus(`Proxy validation failed: ${e}`);
    }
  });

  $("#export-cookies").addEventListener("click", async () => {
    const id = $("#cookie-profile-id").value.trim();
    if (!id) {
      setStatus("Enter a profile ID to export cookies");
      return;
    }
    try {
      const data = await api("/api/v1/cookie_export", { method: "POST", body: JSON.stringify({ profile_id: id }) });
      $("#cookie-result").textContent = JSON.stringify(data, null, 2);
      setStatus(`Exported cookies for ${id}`);
    } catch (e) {
      setStatus(`Cookie export failed: ${e}`);
    }
  });

  $("#import-cookies").addEventListener("click", async () => {
    const id = $("#cookie-profile-id").value.trim();
    const raw = $("#import-cookies-json").value.trim();
    if (!id || !raw) {
      setStatus("Enter profile ID and cookies JSON");
      return;
    }
    let cookies;
    try {
      cookies = JSON.parse(raw);
    } catch (e) {
      setStatus(`Invalid cookies JSON: ${e}`);
      return;
    }
    try {
      await api("/api/v1/cookie_import", { method: "POST", body: JSON.stringify({ profile_id: id, cookies }) });
      setStatus("Cookies imported");
      $("#import-cookies-json").value = "";
    } catch (e) {
      setStatus(`Cookie import failed: ${e}`);
    }
  });

  try {
    $("#api-base").textContent = `REST API: ${await apiBase()}/api/v1`;
  } catch {
    $("#api-base").textContent = "REST API: unavailable";
  }

  showStorageStatus();
  refreshOrg().then(refreshProfiles);
  refreshSessions();
});

// Tells the user when the encrypted profile store could not be opened, so an
// empty list is never mistaken for "no profiles".
async function showStorageStatus() {
  const banner = $("#storage-banner");
  try {
    const status = await invoke("get_storage_status");
    const message = status.error || status.warning;
    storageLocked = !status.ok;
    $("#open-create").disabled = storageLocked;
    renderProfiles();
    banner.hidden = !message;
    banner.textContent = message || "";
    banner.classList.toggle("warning", status.ok);
  } catch (e) {
    banner.hidden = false;
    banner.classList.remove("warning");
    banner.textContent = `Could not check the profile store: ${e}`;
  }
}

// ---------------- Custom masking (fingerprint) ----------------

const MASKING_MODES = [
  ["", "Natural (default)"],
  ["Natural", "Natural"],
  ["Mask", "Mask"],
  ["Custom", "Custom"],
  ["Disabled", "Disabled"],
];
const CANVAS_MODES = [
  ["", "Mask (default)"],
  ["Mask", "Mask"],
  ["Natural", "Natural"],
  ["Disabled", "Disabled"],
  ["Random", "Random (per session)"],
  ["Persistent", "Persistent (per seed)"],
  ["Low", "Low"],
];
const POPUP_MODES = [
  ["", "Prompt (default)"],
  ["Prompt", "Prompt"],
  ["Allow", "Allow"],
  ["Block", "Block"],
];

function fillFlagSelects() {
  for (const sel of document.querySelectorAll("select.masking-flag")) {
    sel.textContent = "";
    for (const [value, label] of MASKING_MODES) {
      sel.appendChild(el("option", { text: label, attrs: { value } }));
    }
  }
  for (const sel of document.querySelectorAll("select.canvas-flag")) {
    sel.textContent = "";
    for (const [value, label] of CANVAS_MODES) {
      sel.appendChild(el("option", { text: label, attrs: { value } }));
    }
  }
  for (const sel of document.querySelectorAll("select.popup-flag")) {
    sel.textContent = "";
    for (const [value, label] of POPUP_MODES) {
      sel.appendChild(el("option", { text: label, attrs: { value } }));
    }
  }
}

function fpText(id) {
  const v = $(id).value.trim();
  return v || undefined;
}

function fpNum(id) {
  const v = $(id).value.trim();
  return v ? Number(v) : undefined;
}

// Every fingerprint field the form shows. Anything else in a stored
// fingerprint, such as a randomized identity's user agent and client hints, is
// kept as it is when the form is saved.
const FORM_FINGERPRINT_KEYS = [
  "os_type", "platform", "hardware_concurrency", "device_memory", "max_touch_points", "vendor",
  "languages", "accept_languages", "timezone", "screen_width", "screen_height", "pixel_ratio",
  "color_depth", "webgl_vendor", "webgl_renderer", "audio_inputs", "audio_outputs", "video_inputs",
  "fonts", "webrtc_policy", "webrtc_public_ip", "webrtc_local_ip", "seed",
];

function withoutFormFields(existing) {
  const kept = { ...(existing || {}) };
  for (const key of FORM_FINGERPRINT_KEYS) delete kept[key];
  if (kept.flags) {
    const shown = new Set([...document.querySelectorAll("[data-flag]")].map((sel) => sel.dataset.flag));
    kept.flags = Object.fromEntries(Object.entries(kept.flags).filter(([flag]) => !shown.has(flag)));
  }
  return kept;
}

function buildFingerprint(existing) {
  if (!$("#masking-enabled").checked) return undefined;

  const fp = {};
  const os = fpText("#fp-os");
  if (os) fp.os_type = os;
  const platform = fpText("#fp-platform");
  if (platform) fp.platform = platform;
  const cores = fpNum("#fp-cores");
  if (cores != null) fp.hardware_concurrency = cores;
  const memory = fpNum("#fp-memory");
  if (memory != null) fp.device_memory = memory;
  const touch = fpNum("#fp-touch");
  if (touch != null) fp.max_touch_points = touch;
  const vendor = fpText("#fp-vendor");
  if (vendor) fp.vendor = vendor;

  const languages = fpText("#fp-languages");
  if (languages) fp.languages = languages;
  const accept = fpText("#fp-accept");
  if (accept) fp.accept_languages = accept;
  const timezone = fpText("#fp-timezone");
  if (timezone) fp.timezone = timezone;

  const screenW = fpNum("#fp-screen-w");
  if (screenW != null) fp.screen_width = screenW;
  const screenH = fpNum("#fp-screen-h");
  if (screenH != null) fp.screen_height = screenH;
  const pixelRatio = fpNum("#fp-pixel-ratio");
  if (pixelRatio != null) fp.pixel_ratio = pixelRatio;
  const colorDepth = fpNum("#fp-color-depth");
  if (colorDepth != null) fp.color_depth = colorDepth;
  const webglVendor = fpText("#fp-webgl-vendor");
  if (webglVendor) fp.webgl_vendor = webglVendor;
  const webglRenderer = fpText("#fp-webgl-renderer");
  if (webglRenderer) fp.webgl_renderer = webglRenderer;

  const audioIn = fpNum("#fp-audio-in");
  if (audioIn != null) fp.audio_inputs = audioIn;
  const audioOut = fpNum("#fp-audio-out");
  if (audioOut != null) fp.audio_outputs = audioOut;
  const videoIn = fpNum("#fp-video-in");
  if (videoIn != null) fp.video_inputs = videoIn;

  const fontsRaw = $("#fp-fonts").value.trim();
  if (fontsRaw) {
    fp.fonts = fontsRaw.split("\n").map((f) => f.trim()).filter(Boolean);
  }

  const webrtcPolicy = fpText("#fp-webrtc-policy");
  if (webrtcPolicy) fp.webrtc_policy = webrtcPolicy;
  const webrtcPublic = fpText("#fp-webrtc-public");
  if (webrtcPublic) fp.webrtc_public_ip = webrtcPublic;
  const webrtcLocal = fpText("#fp-webrtc-local");
  if (webrtcLocal) fp.webrtc_local_ip = webrtcLocal;
  const seed = fpNum("#fp-seed");
  if (seed != null) fp.seed = seed;

  const flags = {};
  for (const sel of document.querySelectorAll("[data-flag]")) {
    if (sel.value) flags[sel.dataset.flag] = sel.value;
  }
  if (Object.keys(flags).length > 0) fp.flags = flags;

  const kept = withoutFormFields(existing);
  const merged = { ...kept, ...fp };
  if (kept.flags || fp.flags) merged.flags = { ...kept.flags, ...fp.flags };
  return Object.keys(merged).length > 0 ? merged : undefined;
}

function setVal(id, value) {
  $(id).value = value == null ? "" : value;
}

function fillFingerprint(fp) {
  const enabled = Boolean(fp);
  $("#masking-enabled").checked = enabled;
  $("#masking-details").open = enabled;

  setVal("#fp-os", fp?.os_type);
  setVal("#fp-platform", fp?.platform);
  setVal("#fp-cores", fp?.hardware_concurrency);
  setVal("#fp-memory", fp?.device_memory);
  setVal("#fp-touch", fp?.max_touch_points);
  setVal("#fp-vendor", fp?.vendor);
  setVal("#fp-languages", fp?.languages);
  setVal("#fp-accept", fp?.accept_languages);
  setVal("#fp-timezone", fp?.timezone);
  setVal("#fp-screen-w", fp?.screen_width);
  setVal("#fp-screen-h", fp?.screen_height);
  setVal("#fp-pixel-ratio", fp?.pixel_ratio);
  setVal("#fp-color-depth", fp?.color_depth);
  setVal("#fp-webgl-vendor", fp?.webgl_vendor);
  setVal("#fp-webgl-renderer", fp?.webgl_renderer);
  setVal("#fp-audio-in", fp?.audio_inputs);
  setVal("#fp-audio-out", fp?.audio_outputs);
  setVal("#fp-video-in", fp?.video_inputs);
  setVal("#fp-fonts", (fp?.fonts || []).join("\n"));
  setVal("#fp-webrtc-policy", fp?.webrtc_policy);
  setVal("#fp-webrtc-public", fp?.webrtc_public_ip);
  setVal("#fp-webrtc-local", fp?.webrtc_local_ip);
  setVal("#fp-seed", fp?.seed);

  const flags = fp?.flags || {};
  for (const sel of document.querySelectorAll("[data-flag]")) {
    sel.value = flags[sel.dataset.flag] || "";
  }
}
