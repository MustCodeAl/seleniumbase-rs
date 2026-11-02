-- Chrome for Testing, as far as the chromedriver plugin needs it: the index of
-- builds, the platform names it uses, and a check of the URL a download comes from.
local http = require("http")
local json = require("json")

local M = {}

-- The index of every build with its downloads, oldest first.
M.index_url = "https://googlechromelabs.github.io/chrome-for-testing/known-good-versions-with-downloads.json"

-- The host the index names for its downloads. A URL anywhere else is refused.
M.download_host = "storage.googleapis.com"

-- Chrome for Testing's platform names, by the "os/arch" that RUNTIME reports.
-- Builds exist for these and for nothing else.
local PLATFORMS = {
    ["linux/amd64"] = "linux64",
    ["linux/arm64"] = "linux-arm64",
    ["darwin/amd64"] = "mac-x64",
    ["darwin/arm64"] = "mac-arm64",
    ["windows/amd64"] = "win64",
    ["windows/x86"] = "win32",
}

-- The Chrome for Testing platform of `runtime` (the RUNTIME global, or a table
-- shaped like it). Raises an error that says why when there is none.
function M.platform(runtime)
    local key = runtime.osType .. "/" .. runtime.archType
    local platform = PLATFORMS[key]
    if platform == nil then
        error(
            "Chrome for Testing publishes no chromedriver for "
                .. key
                .. " (it has linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64 and windows/x86). "
                .. "Install your system's chromedriver package instead, and put it on the PATH."
        )
    end
    if runtime.osType == "linux" and runtime.envType == "musl" then
        error(
            "Chrome for Testing builds chromedriver for glibc, and this system uses musl, so it would not start. "
                .. "Install your system's chromedriver package instead (on Alpine: chromium-chromedriver)."
        )
    end
    return platform
end

-- The index, decoded. Raises an error when it cannot be fetched or has no list of builds.
function M.fetch_index()
    local resp = http.get({ url = M.index_url })
    if resp.status_code ~= 200 then
        error("fetching " .. M.index_url .. " failed: HTTP " .. tostring(resp.status_code))
    end
    local index = json.decode(resp.body)
    if type(index) ~= "table" or type(index.versions) ~= "table" then
        error(M.index_url .. " does not have the list of versions this plugin reads")
    end
    return index
end

-- The URL of the chromedriver archive of `platform` in one entry of the index,
-- or nil when that build has none. Builds before 115 have no chromedriver, and
-- linux-arm64 starts at 153.
function M.download_for(entry, platform)
    local downloads = entry.downloads and entry.downloads.chromedriver
    if type(downloads) ~= "table" then
        return nil
    end
    for _, download in ipairs(downloads) do
        if download.platform == platform then
            return download.url
        end
    end
    return nil
end

-- The version of the oldest build in `index` that has a chromedriver for
-- `platform`, or nil when none has. The index lists builds oldest first. It
-- tells a user of a platform that Chrome for Testing covers only from some
-- build on (linux-arm64) where the builds start.
function M.oldest_with_download(index, platform)
    for _, entry in ipairs(index.versions) do
        if M.download_for(entry, platform) then
            return entry.version
        end
    end
    return nil
end


-- `url` when it is an https URL on the host the index names for its downloads
-- and names a zip archive, else an error. The URL comes from the index and is
-- never built here.
function M.checked_url(url)
    if type(url) ~= "string" then
        error("the index gives no download URL")
    end
    -- The part between "https://" and the first "/" is the whole authority, so a
    -- user name before an "@" or a port after the host makes it differ.
    local authority = url:match("^https://([^/]+)/")
    if authority ~= M.download_host then
        error("refusing " .. url .. ": downloads must come over https from " .. M.download_host)
    end
    if not url:match("%.zip$") then
        error("refusing " .. url .. ": expected a .zip archive")
    end
    return url
end

return M
