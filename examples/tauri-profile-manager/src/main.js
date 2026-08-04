const { invoke } = window.__TAURI__.core;

const statusEl = document.querySelector("#status");

function setStatus(msg) {
  statusEl.textContent = msg;
}

async function refreshProfiles() {
  const profiles = await invoke("list_profiles");
  const list = document.querySelector("#profile-list");
  list.innerHTML = "";
  for (const p of profiles) {
    const li = document.createElement("li");
    const geo = p.latitude != null && p.longitude != null
      ? `📍 ${p.latitude.toFixed(4)}, ${p.longitude.toFixed(4)}`
      : "";
    const mlBadge = p.external_profile ? `<span class="tag external">external</span>` : "";
    li.innerHTML = `
      <div class="profile-row">
        <strong>${p.name}</strong>
        <span class="muted">${p.container_url}</span>
        <span class="tag">${p.mode || "webdriver"}</span>
        ${mlBadge}
        <span class="muted">${geo}</span>
      </div>
      <div class="actions">
        <input type="text" class="start-url" placeholder="Start URL" />
        <button class="launch">Launch</button>
        <button class="delete">Delete</button>
        <button class="clone">Clone</button>
        <button class="export">Export</button>
      </div>
    `;
    li.querySelector(".launch").addEventListener("click", async () => {
      const url = li.querySelector(".start-url").value || undefined;
      setStatus(`Launching ${p.name}...`);
      try {
        const info = await invoke("launch_profile", { id: p.id, startUrl: url });
        setStatus(`Launched ${info.profile_name} → ${info.session_id}`);
        refreshSessions();
      } catch (e) {
        setStatus(`Launch failed: ${e}`);
      }
    });
    li.querySelector(".delete").addEventListener("click", async () => {
      await invoke("delete_profile", { id: p.id });
      refreshProfiles();
    });
    li.querySelector(".clone").addEventListener("click", async () => {
      const base = await apiBase();
      setStatus(`Cloning ${p.name}...`);
      try {
        const res = await fetch(`${base}/api/v1/profiles/${p.id}/clone`, { method: "POST" });
        const body = await res.json();
        const cloneId = body.data.id;
        const newName = `${p.name} (clone)`;
        await fetch(`${base}/api/v1/profiles/${cloneId}`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ name: newName }),
        });
        setStatus(`Cloned as ${newName}`);
        refreshProfiles();
      } catch (e) {
        setStatus(`Clone failed: ${e}`);
      }
    });
    li.querySelector(".export").addEventListener("click", async () => {
      const base = await apiBase();
      try {
        const res = await fetch(`${base}/api/v1/profiles/${p.id}/export`);
        const body = await res.json();
        const blob = new Blob([JSON.stringify(body.data, null, 2)], { type: "application/json" });
        const a = document.createElement("a");
        a.href = URL.createObjectURL(blob);
        a.download = `${p.name}-profile.json`;
        a.click();
        URL.revokeObjectURL(a.href);
        setStatus(`Exported ${p.name}`);
      } catch (e) {
        setStatus(`Export failed: ${e}`);
      }
    });
    list.appendChild(li);
  }
}

async function apiBase() {
  return invoke("get_api_base");
}

async function refreshTags() {
  const base = await apiBase();
  try {
    const [tagsRes, foldersRes] = await Promise.all([
      fetch(`${base}/api/v1/tags`),
      fetch(`${base}/api/v1/folders`),
    ]);
    const tagsBody = await tagsRes.json();
    const foldersBody = await foldersRes.json();

    const tagList = document.querySelector("#tag-list");
    tagList.innerHTML = "";
    for (const t of tagsBody.data || []) {
      const li = document.createElement("li");
      li.textContent = `${t.name} (${t.color})`;
      tagList.appendChild(li);
    }

    const folderList = document.querySelector("#folder-list");
    folderList.innerHTML = "";
    for (const f of foldersBody.data || []) {
      const li = document.createElement("li");
      li.textContent = f.name;
      folderList.appendChild(li);
    }
  } catch (e) {
    setStatus(`Tags refresh failed: ${e}`);
  }
}

async function refreshSessions() {
  const sessions = await invoke("list_sessions");
  const list = document.querySelector("#session-list");
  list.innerHTML = "";
  for (const s of sessions) {
    const li = document.createElement("li");
    li.innerHTML = `
      <div class="session-row">
        <strong>${s.profile_name}</strong>
        <span class="muted">${s.container_url}</span>
      </div>
      <div class="actions">
        <input type="text" class="nav-url" placeholder="Navigate to URL" />
        <button class="navigate">Go</button>
        <button class="screenshot">Screenshot</button>
      </div>
      <div class="actions">
        <input type="number" step="any" class="lat" placeholder="Lat" />
        <input type="number" step="any" class="lon" placeholder="Lon" />
        <input type="number" step="any" class="acc" placeholder="Accuracy m" />
        <button class="set-geo">Set Geo</button>
        <button class="close">Close</button>
      </div>
    `;
    li.querySelector(".navigate").addEventListener("click", async () => {
      const url = li.querySelector(".nav-url").value;
      if (!url) return;
      setStatus(`Navigating ${s.session_id}...`);
      try {
        await invoke("navigate_session", { sessionId: s.session_id, url });
        setStatus("Navigation complete");
      } catch (e) {
        setStatus(`Navigation failed: ${e}`);
      }
    });
    li.querySelector(".screenshot").addEventListener("click", async () => {
      setStatus("Taking screenshot...");
      try {
        const path = await invoke("take_screenshot", { sessionId: s.session_id });
        setStatus(`Screenshot saved: ${path}`);
      } catch (e) {
        setStatus(`Screenshot failed: ${e}`);
      }
    });
    li.querySelector(".set-geo").addEventListener("click", async () => {
      const lat = Number(li.querySelector(".lat").value);
      const lon = Number(li.querySelector(".lon").value);
      const acc = li.querySelector(".acc").value;
      if (Number.isNaN(lat) || Number.isNaN(lon)) {
        setStatus("Enter valid latitude and longitude");
        return;
      }
      setStatus(`Setting geolocation ${lat}, ${lon}...`);
      try {
        await invoke("set_session_geolocation", {
          sessionId: s.session_id,
          latitude: lat,
          longitude: lon,
          accuracy: acc ? Number(acc) : undefined,
        });
        setStatus("Geolocation updated");
      } catch (e) {
        setStatus(`Geolocation failed: ${e}`);
      }
    });
    li.querySelector(".close").addEventListener("click", async () => {
      await invoke("close_session", { sessionId: s.session_id });
      refreshSessions();
    });
    list.appendChild(li);
  }
}

window.addEventListener("DOMContentLoaded", async () => {
  document.querySelector("#profile-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    const parseNum = (id) => {
      const el = document.querySelector(id);
      const v = el.value.trim();
      return v ? Number(v) : undefined;
    };
    const newProfile = {
      name: document.querySelector("#profile-name").value,
      containerUrl: document.querySelector("#container-url").value,
      userAgent: document.querySelector("#user-agent").value || undefined,
      proxy: document.querySelector("#proxy").value || undefined,
      locale: document.querySelector("#locale").value || undefined,
      latitude: parseNum("#latitude"),
      longitude: parseNum("#longitude"),
      accuracy: parseNum("#accuracy"),
      headless: document.querySelector("#headless").checked,
    };
    await invoke("create_profile", { new: newProfile });
    e.target.reset();
    refreshProfiles();
  });

  document.querySelector("#refresh-profiles").addEventListener("click", refreshProfiles);
  document.querySelector("#refresh-sessions").addEventListener("click", refreshSessions);
  document.querySelector("#refresh-tags").addEventListener("click", refreshTags);

  document.querySelector("#clone-profile").addEventListener("click", async () => {
    const sourceId = document.querySelector("#clone-source-id").value.trim();
    const newName = document.querySelector("#clone-new-name").value.trim();
    if (!sourceId || !newName) {
      setStatus("Enter source profile ID and new name");
      return;
    }
    const base = await apiBase();
    setStatus(`Cloning ${sourceId}...`);
    try {
      const res = await fetch(`${base}/api/v1/profiles/${sourceId}/clone`, { method: "POST" });
      const body = await res.json();
      const cloneId = body.data.id;
      await fetch(`${base}/api/v1/profiles/${cloneId}`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ name: newName }),
      });
      setStatus(`Cloned to ${newName}`);
      document.querySelector("#clone-source-id").value = "";
      document.querySelector("#clone-new-name").value = "";
      refreshProfiles();
    } catch (e) {
      setStatus(`Clone failed: ${e}`);
    }
  });

  document.querySelector("#export-profile").addEventListener("click", async () => {
    const id = document.querySelector("#export-profile-id").value.trim();
    if (!id) {
      setStatus("Enter a profile ID to export");
      return;
    }
    const base = await apiBase();
    try {
      const res = await fetch(`${base}/api/v1/profiles/${id}/export`);
      const body = await res.json();
      const blob = new Blob([JSON.stringify(body.data, null, 2)], { type: "application/json" });
      const a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = `profile-${id}.json`;
      a.click();
      URL.revokeObjectURL(a.href);
      setStatus(`Exported profile ${id}`);
    } catch (e) {
      setStatus(`Export failed: ${e}`);
    }
  });

  document.querySelector("#import-profile").addEventListener("click", async () => {
    const raw = document.querySelector("#import-profile-json").value.trim();
    if (!raw) {
      setStatus("Paste an external profile JSON to import");
      return;
    }
    const base = await apiBase();
    try {
      const res = await fetch(`${base}/api/v1/profiles/import`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: raw,
      });
      const body = await res.json();
      setStatus(`Imported external profile ${body.data.name}`);
      document.querySelector("#import-profile-json").value = "";
      refreshProfiles();
    } catch (e) {
      setStatus(`External profile import failed: ${e}`);
    }
  });

  document.querySelector("#import-profile").addEventListener("click", async () => {
    const raw = document.querySelector("#import-profile-json").value.trim();
    if (!raw) {
      setStatus("Paste profile JSON to import");
      return;
    }
    const base = await apiBase();
    try {
      const res = await fetch(`${base}/api/v1/profiles/import`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: raw,
      });
      const body = await res.json();
      setStatus(`Imported profile ${body.data.name}`);
      document.querySelector("#import-profile-json").value = "";
      refreshProfiles();
    } catch (e) {
      setStatus(`Import failed: ${e}`);
    }
  });

  document.querySelector("#validate-proxy").addEventListener("click", async () => {
    const base = await apiBase();
    const payload = {
      type: document.querySelector("#proxy-type").value,
      host: document.querySelector("#proxy-host").value.trim(),
      port: Number(document.querySelector("#proxy-port").value),
      username: document.querySelector("#proxy-username").value || undefined,
      password: document.querySelector("#proxy-password").value || undefined,
    };
    if (!payload.host || Number.isNaN(payload.port)) {
      setStatus("Enter proxy host and port");
      return;
    }
    setStatus("Validating proxy...");
    try {
      const res = await fetch(`${base}/api/v1/proxy/validate`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(payload),
      });
      const body = await res.json();
      document.querySelector("#proxy-result").textContent = JSON.stringify(body.data || body.status, null, 2);
      setStatus("Proxy validation complete");
    } catch (e) {
      setStatus(`Proxy validation failed: ${e}`);
    }
  });

  document.querySelector("#export-cookies").addEventListener("click", async () => {
    const id = document.querySelector("#cookie-profile-id").value.trim();
    if (!id) {
      setStatus("Enter a profile ID to export cookies");
      return;
    }
    const base = await apiBase();
    try {
      const res = await fetch(`${base}/api/v1/cookie_export`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ profile_id: id }),
      });
      const body = await res.json();
      document.querySelector("#cookie-result").textContent = JSON.stringify(body.data || body.status, null, 2);
      setStatus(`Exported cookies for ${id}`);
    } catch (e) {
      setStatus(`Cookie export failed: ${e}`);
    }
  });

  document.querySelector("#import-cookies").addEventListener("click", async () => {
    const id = document.querySelector("#cookie-profile-id").value.trim();
    const raw = document.querySelector("#import-cookies-json").value.trim();
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
    const base = await apiBase();
    try {
      const res = await fetch(`${base}/api/v1/cookie_import`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ profile_id: id, cookies }),
      });
      const body = await res.json();
      setStatus(body.status?.message || "Cookies imported");
      document.querySelector("#import-cookies-json").value = "";
    } catch (e) {
      setStatus(`Cookie import failed: ${e}`);
    }
  });

  document.querySelector("#add-tag").addEventListener("click", async () => {
    const name = document.querySelector("#new-tag").value;
    if (!name) return;
    const base = await apiBase();
    try {
      await fetch(`${base}/api/v1/tags`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ name }),
      });
      document.querySelector("#new-tag").value = "";
      refreshTags();
    } catch (e) {
      setStatus(`Add tag failed: ${e}`);
    }
  });

  document.querySelector("#add-folder").addEventListener("click", async () => {
    const name = document.querySelector("#new-folder").value;
    if (!name) return;
    const base = await apiBase();
    try {
      await fetch(`${base}/api/v1/folders`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ name }),
      });
      document.querySelector("#new-folder").value = "";
      refreshTags();
    } catch (e) {
      setStatus(`Add folder failed: ${e}`);
    }
  });

  try {
    document.querySelector("#api-base").textContent = `REST API: ${await apiBase()}/api/v1`;
  } catch {
    document.querySelector("#api-base").textContent = "REST API: unavailable";
  }

  refreshProfiles();
  refreshSessions();
  refreshTags();
});
