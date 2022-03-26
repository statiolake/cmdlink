function _G.is_windows()
    return os.getenv 'OS' == 'Windows_NT'
end

function _G.find_var_name(envvars, name)
    for k, _ in pairs(envvars) do
        if k == name or (is_windows and k:lower() == name:lower()) then
            return k
        end
    end
    return nil
end

function _G.add_to_variable(envvars, key, value, sep)
    key = find_var_name(envvars, key)
    if envvars[key] ~= nil and envvars[key] ~= '' then
        sep = sep or (is_windows() and ';' or ':')
        envvars[key] = string.format('%s%s%s', envvars[key], sep, value)
    else
        envvars[key] = value
    end
end
