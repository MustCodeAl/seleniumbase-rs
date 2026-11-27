local TARGETS = {
    ["darwin/arm64"] = { triple = "aarch64-apple-darwin", ext = "tgz" },
    ["darwin/amd64"] = { triple = "x86_64-apple-darwin", ext = "tgz" },
    ["linux/amd64"] = { triple = "x86_64-unknown-linux-gnu", ext = "tgz" },
    ["linux/arm64"] = { triple = "aarch64-unknown-linux-gnu", ext = "tgz" },
    ["windows/amd64"] = { triple = "x86_64-pc-windows-msvc", ext = "zip" },
}

function PLUGIN:PreInstall(ctx)
    local key = RUNTIME.osType .. "/" .. RUNTIME.archType
    local target = TARGETS[key]
    if target == nil then
        error("sbrs has no prebuilt release archive for " .. key)
    end

    local tag = "v" .. ctx.version:gsub("^v", "")
    local archive = "seleniumbase-rs-" .. tag .. "-" .. target.triple .. "." .. target.ext
    local url = "https://github.com/MustCodeAl/seleniumbase-rs/releases/download/" .. tag .. "/" .. archive
    return {
        version = ctx.version,
        url = url,
    }
end
