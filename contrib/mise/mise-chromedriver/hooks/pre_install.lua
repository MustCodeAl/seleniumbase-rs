local cft = require("cft")

-- Tells mise which archive to download. mise downloads it, unpacks it into the
-- install directory and runs nothing from it. The index lists no checksums, so
-- none is returned.
function PLUGIN:PreInstall(ctx)
    local platform = cft.platform(RUNTIME)
    local index = cft.fetch_index()
    for _, entry in ipairs(index.versions) do
        if entry.version == ctx.version then
            local url = cft.download_for(entry, platform)
            if url == nil then
                -- linux-arm64 is the case this is for: its builds start at 153.
                local oldest = cft.oldest_with_download(index, platform)
                local hint = ""
                if oldest ~= nil then
                    hint = " (the oldest build it has for " .. platform .. " is " .. oldest .. ")"
                end
                error(
                    "Chrome for Testing has no chromedriver "
                        .. ctx.version
                        .. " for "
                        .. platform
                        .. hint
                        .. ". `mise ls-remote chromedriver` lists the builds that have one."
                )
            end
            return { version = ctx.version, url = cft.checked_url(url) }
        end
    end
    error("Chrome for Testing does not list chromedriver " .. ctx.version)
end
