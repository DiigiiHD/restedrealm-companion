-- Read-only metadata inspection. Does not print names, text, or account details.
local file = assert(arg[1], "pass a RestedRealmCollector.lua SavedVariables file")
local firstSeq = tonumber(arg[2]) or 1
local env = {}
assert(loadfile(file, "t", env))()
local db = assert(env.RestedRealmCollectorDB)
for _, observation in ipairs(db.records or {}) do
    if observation.seq >= firstSeq then
        local data = observation.data or {}
        print(string.format("seq=%d kind=%s id=%s title=%s level=%s stage=%s event=%s objectives=%s pin=%s",
            observation.seq, tostring(observation.kind), tostring(data.id),
            tostring(data.title), tostring(data.level), tostring(data.stage),
            tostring(data.event), tostring(data.objectives and #data.objectives),
            tostring(data.mapPin and data.mapPin.mapID)))
    end
    if observation.seq >= firstSeq and observation.kind == "loot_window" then
        for _, item in ipairs(observation.data.items or {}) do
            print(string.format("seq=%d item=%s qty=%s source=%s identitySchema=%s",
                observation.seq, tostring(item.id), tostring(item.quantity),
                tostring(item.sourceID), tostring(observation.identitySchema)))
        end
    end
end
