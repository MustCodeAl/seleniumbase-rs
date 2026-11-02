-- The archive holds one folder with chromedriver at its top; mise unpacks the
-- folder's contents into the install directory, so chromedriver is there and
-- there is no bin/ folder.
function PLUGIN:EnvKeys(ctx)
    return {
        { key = "PATH", value = ctx.path },
    }
end
