-- Read-only, conservative links between a visible loot slot and later quest progress.
-- Usage: lua tools/audit_loot_quest_links.lua <trusted SavedVariables file>
-- Only open a file produced by the local game client: loadfile executes Lua.
local path = assert(arg[1], "pass the collector SavedVariables path")
local env = {}
assert(loadfile(path, "t", env))()
local records = assert(env.RestedRealmCollectorDB).records or {}
local latestObjectives, recentWindows, cleared, printed = {}, {}, {}, {}

local function objectiveName(label)
    if type(label) ~= "string" then return nil end
    local name = string.match(label, "^%d+/%d+%s+(.+)$") or label
    return string.lower(name)
end

for _, observation in ipairs(records) do
    local data = observation.data or {}
    local at = observation.observedAt
    if observation.kind == "loot_window" then
        recentWindows[#recentWindows + 1] = observation
    elseif observation.kind == "loot_slot_cleared" then
        for i = #recentWindows, 1, -1 do
            local window = recentWindows[i]
            if at and window.observedAt and at - window.observedAt >= 0
                and at - window.observedAt <= 10 then
                local matched = false
                for _, item in ipairs(window.data.items or {}) do
                    if item.id == data.itemID and item.sourceID == data.sourceID then
                        cleared[#cleared + 1] = {
                            at = at, seq = observation.seq, item = item,
                        }
                        matched = true
                        break
                    end
                end
                if matched then break end
            end
        end
    elseif observation.kind == "quest_objectives" and data.id then
        local previous = latestObjectives[data.id]
        if previous and at then
            for index, objective in ipairs(data.objectives or {}) do
                local before = previous[index]
                local after = objective.fulfilled
                local name = objectiveName(objective.text)
                if before and type(before.fulfilled) == "number"
                    and type(after) == "number" and after > before.fulfilled
                    and name == before.name then
                    for _, loot in ipairs(cleared) do
                        if loot.at and at - loot.at >= 0 and at - loot.at <= 10
                            and objectiveName(loot.item.name) == name then
                            local key = table.concat({ tostring(loot.seq),
                                tostring(data.id), tostring(index) }, ":")
                            if not printed[key] then
                                printed[key] = true
                                print(string.format(
                                    "candidate loot_seq=%d item=%s source=%s quest=%d objective=%d progress=%d->%d delay_seconds=%d evidence=visible_loot+slot_clear+matching_objective_delta",
                                    loot.seq, tostring(loot.item.id),
                                    tostring(loot.item.sourceID), data.id, index,
                                    before.fulfilled, after, at - loot.at))
                            end
                        end
                    end
                end
            end
        end
        local snapshot = {}
        for index, objective in ipairs(data.objectives or {}) do
            snapshot[index] = { name = objectiveName(objective.text),
                fulfilled = objective.fulfilled }
        end
        latestObjectives[data.id] = snapshot
    end
end
