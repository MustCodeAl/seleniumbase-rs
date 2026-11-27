local http = require("http")
local json = require("json")

local RELEASES_URL = "https://api.github.com/repos/MustCodeAl/seleniumbase-rs/releases?per_page=30"

local TARGETS = {
    ["darwin/arm64"] = { triple = "aarch64-apple-darwin", ext = "tgz" },
    ["darwin/amd64"] = { triple = "x86_64-apple-darwin", ext = "tgz" },
    ["linux/amd64"] = { triple = "x86_64-unknown-linux-gnu", ext = "tgz" },
    ["linux/arm64"] = { triple = "aarch64-unknown-linux-gnu", ext = "tgz" },
    ["windows/amd64"] = { triple = "x86_64-pc-windows-msvc", ext = "zip" },
}

function PLUGIN:Available(ctx)
    local key = RUNTIME.osType .. "/" .. RUNTIME.archType
    local target = TARGETS[key]
    if target == nil then
        error("sbrs has no prebuilt release archive for " .. key)
    end

    local resp, err = http.get({
        url = RELEASES_URL,
        headers = {
            ["Accept"] = "application/vnd.github+json",
            ["User-Agent"] = "mise-sbrs-plugin",
        },
    })
    if err ~= nil then
        error("failed to fetch sbrs releases: " .. tostring(err))
    end
    if resp.status_code ~= 200 then
        error("GitHub API returned status " .. tostring(resp.status_code))
    end

    local releases = json.decode(resp.body)
    local result = {}
    for _, rel in ipairs(releases) do
        if not rel.draft and type(rel.tag_name) == "string" and type(rel.assets) == "table" then
            local ver = rel.tag_name:gsub("^v", "")
            local wanted = "seleniumbase-rs-" .. rel.tag_name .. "-" .. target.triple .. "." .. target.ext
            for _, asset in ipairs(rel.assets) do
                if asset.name == wanted then
                    table.insert(result, { version = ver })
                    break
                end
            end
        end
    end
    return result
end
