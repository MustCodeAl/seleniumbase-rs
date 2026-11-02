local cft = require("cft")

-- Every build that has a chromedriver for this platform, newest first. The index
-- lists the builds oldest first.
function PLUGIN:Available(ctx)
    local platform = cft.platform(RUNTIME)
    local index = cft.fetch_index()
    local result = {}
    for i = #index.versions, 1, -1 do
        local entry = index.versions[i]
        if cft.download_for(entry, platform) then
            table.insert(result, { version = entry.version })
        end
    end
    if #result == 0 then
        error("Chrome for Testing lists no chromedriver for " .. platform)
    end
    return result
end
