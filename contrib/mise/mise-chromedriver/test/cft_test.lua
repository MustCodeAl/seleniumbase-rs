-- Checks of lib/cft.lua that need neither mise nor the network.
--
-- Run it from the plugin folder with a Lua 5.1 compatible interpreter:
--
--     luajit test/cft_test.lua
--
-- It prints one line per failure and exits with status 1 when there is any.
package.path = "./lib/?.lua;" .. package.path
-- cft.lua loads mise's http and json modules; these checks do not call them.
package.preload["http"] = function()
    return {}
end
package.preload["json"] = function()
    return {}
end

local cft = require("cft")

local failures = 0

local function check(name, condition)
    if not condition then
        failures = failures + 1
        print("FAIL: " .. name)
    end
end

-- The error message when `fn` raises, else nil.
local function message(fn)
    local ok, err = pcall(fn)
    if ok then
        return nil
    end
    return tostring(err)
end

local function platform_of(os_type, arch_type, env_type)
    return cft.platform({ osType = os_type, archType = arch_type, envType = env_type })
end

-- Every platform Chrome for Testing builds chromedriver for.
check("linux amd64", platform_of("linux", "amd64", "gnu") == "linux64")
check("linux arm64", platform_of("linux", "arm64", "gnu") == "linux-arm64")
check("macOS Intel", platform_of("darwin", "amd64") == "mac-x64")
check("macOS Apple silicon", platform_of("darwin", "arm64") == "mac-arm64")
check("Windows 64-bit", platform_of("windows", "amd64") == "win64")
check("Windows 32-bit", platform_of("windows", "x86") == "win32")

-- Anything else is refused with a message that says what to do, not mapped to a build.
local riscv = message(function()
    platform_of("linux", "riscv64", "gnu")
end)
check("riscv64 is refused", riscv ~= nil and riscv:find("linux/riscv64", 1, true) ~= nil)
local windows_arm = message(function()
    platform_of("windows", "arm64")
end)
check("Windows on arm is refused", windows_arm ~= nil and windows_arm:find("windows/arm64", 1, true) ~= nil)
local musl = message(function()
    platform_of("linux", "amd64", "musl")
end)
check("musl is refused", musl ~= nil and musl:find("musl", 1, true) ~= nil)

-- The URL of a download comes from the index, and only https on the named host passes.
local good = "https://storage.googleapis.com/chrome-for-testing-public/150.0.7871.124/mac-arm64/chromedriver-mac-arm64.zip"
check("the Chrome for Testing URL passes", cft.checked_url(good) == good)
local refused = {
    ["plain http"] = "http://storage.googleapis.com/chrome-for-testing-public/1/x/chromedriver-x.zip",
    ["another host"] = "https://example.com/chrome-for-testing-public/1/x/chromedriver-x.zip",
    ["a user name in front of the host"] = "https://storage.googleapis.com@example.com/1/x/chromedriver-x.zip",
    ["a port"] = "https://storage.googleapis.com:8443/1/x/chromedriver-x.zip",
    ["a look-alike host"] = "https://storage.googleapis.com.example.com/1/x/chromedriver-x.zip",
    ["no path"] = "https://storage.googleapis.com",
    ["not an archive"] = "https://storage.googleapis.com/chrome-for-testing-public/1/x/chromedriver-x.sh",
}
for name, url in pairs(refused) do
    check("refused: " .. name, message(function()
        cft.checked_url(url)
    end) ~= nil)
end
check("no URL at all is refused", message(function()
    cft.checked_url(nil)
end) ~= nil)

-- Finding a platform's archive in one entry of the index.
local entry = {
    version = "153.0.8001.0",
    downloads = {
        chrome = { { platform = "linux64", url = "chrome" } },
        chromedriver = {
            { platform = "linux64", url = "linux64.zip" },
            { platform = "linux-arm64", url = "arm64.zip" },
        },
    },
}
check("the archive of a platform", cft.download_for(entry, "linux-arm64") == "arm64.zip")
check("a platform the build lacks", cft.download_for(entry, "win64") == nil)
check("a build before 115 has no chromedriver", cft.download_for({ version = "113.0.5672.0", downloads = { chrome = {} } }, "linux64") == nil)
check("an entry without downloads", cft.download_for({ version = "1" }, "linux64") == nil)

-- Where a platform's builds start: Chrome for Testing has linux-arm64 only from 153.
local index = {
    versions = {
        { version = "150.0.1.0", downloads = { chromedriver = { { platform = "linux64", url = "a.zip" } } } },
        { version = "153.0.8001.0", downloads = { chromedriver = { { platform = "linux64", url = "b.zip" }, { platform = "linux-arm64", url = "c.zip" } } } },
        { version = "154.0.1.0", downloads = { chromedriver = { { platform = "linux-arm64", url = "d.zip" } } } },
    },
}
check("the oldest build of linux-arm64", cft.oldest_with_download(index, "linux-arm64") == "153.0.8001.0")
check("the oldest build of linux64", cft.oldest_with_download(index, "linux64") == "150.0.1.0")
check("a platform no build has", cft.oldest_with_download(index, "win64") == nil)


if failures > 0 then
    os.exit(1)
end
print("ok")
