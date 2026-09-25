-- Local, read-only field audit. Prints counts and IDs, never names or prose.
-- Usage: lua tools/audit_save.lua <SavedVariables file> [quest ID]
-- Only open a file produced by the local game client: loadfile executes Lua.
local path = assert(arg[1], "pass the collector SavedVariables path")
local focusID = tonumber(arg[2])
local env = {}
assert(loadfile(path, "t", env))()
local db = assert(env.RestedRealmCollectorDB)
local records = db.records or {}
local counts, versions = {}, {}
local latestQuest, latestObjectives, latestLogText, latestReputation
local lootQuestLinks = 0
local recent = { npcInteractions = 0, subtitles = 0, itemCountIncreases = 0,
    professionCatalogs = 0, areaEdges = 0, areaEntries = 0, areaExits = 0 }

for _, observation in ipairs(records) do
    local kind = tostring(observation.kind)
    counts[kind] = (counts[kind] or 0) + 1
    local version = tostring(observation.collectorVersion or "old")
    versions[version] = (versions[version] or 0) + 1
    local data = observation.data or {}
    local minor = tonumber(string.match(version, "^0%.1%.(%d+)%-probe$"))
    if minor and minor >= 9 then
        if type(data.npc) == "table" then
            recent.npcInteractions = recent.npcInteractions + 1
            if data.npc.subtitle then recent.subtitles = recent.subtitles + 1 end
        end
        if kind == "item_count_increase" then
            recent.itemCountIncreases = recent.itemCountIncreases + 1
        elseif kind == "profession_catalog" then
            recent.professionCatalogs = recent.professionCatalogs + 1
        elseif kind == "quest_area_edge_sample" then
            recent.areaEdges = recent.areaEdges + 1
            if data.playerInsideBlob == true then
                recent.areaEntries = recent.areaEntries + 1
            elseif data.playerInsideBlob == false then
                recent.areaExits = recent.areaExits + 1
            end
        end
    end
    if focusID and kind == "loot_window" then
        for _, item in ipairs(data.items or {}) do
            if item.questID == focusID then lootQuestLinks = lootQuestLinks + 1 end
        end
    end
    if focusID and data.id == focusID then
        if kind == "quest" then latestQuest = observation end
        if kind == "quest_objectives" then latestObjectives = observation end
        if kind == "quest_log_text" then latestLogText = observation end
        if kind == "quest_reputation" then latestReputation = observation end
    end
end

local function sortedKeys(values)
    local keys = {}
    for key in pairs(values) do keys[#keys + 1] = key end
    table.sort(keys)
    return keys
end

local function show(label, value)
    if value == nil then print(label .. "=unknown")
    else print(label .. "=" .. tostring(value)) end
end

local function countItems(items, kind)
    local count = 0
    for _, item in ipairs(items or {}) do
        if item.type == kind then count = count + 1 end
    end
    return count
end

show("db_version", db.version)
show("records", #records)
show("dropped", db.dropped or 0)
show("handler_errors", db.errors or 0)
for _, kind in ipairs(sortedKeys(counts)) do show("kind." .. kind, counts[kind]) end
for _, version in ipairs(sortedKeys(versions)) do show("version." .. version, versions[version]) end
for _, key in ipairs(sortedKeys(recent)) do show("recent." .. key, recent[key]) end
for _, key in ipairs(sortedKeys(db.apiStatus or {})) do
    show("api." .. key, db.apiStatus[key])
end

if focusID then
    show("focus_quest_id", focusID)
    show("loot_slots_with_quest_id", lootQuestLinks)
    if latestQuest then
        local data = latestQuest.data
        show("quest.seq", latestQuest.seq)
        show("quest.collector_version", latestQuest.collectorVersion)
        show("quest.stage", data.stage)
        show("quest.level", data.level)
        show("quest.giver_id", data.npc and data.npc.id)
        show("quest.giver_subtitle_present", data.npc and data.npc.subtitle ~= nil)
        show("quest.prose_bytes", data.questText and #data.questText)
        show("quest.objective_prose_bytes", data.objectiveText and #data.objectiveText)
        show("quest.required_items", countItems(data.items, "required"))
        show("quest.fixed_items", countItems(data.items, "reward"))
        show("quest.choice_items", countItems(data.items, "choice"))
        show("quest.xp", data.xp)
        show("quest.money_copper", data.moneyCopper)
        show("quest.currencies", data.currencies and #data.currencies)
        show("quest.spell_rewards", data.spellRewardIDs and #data.spellRewardIDs)
        show("quest.text_truncated", data.textTruncated and true or false)
    else
        print("quest.record=unknown")
    end
    if latestObjectives then
        local data = latestObjectives.data
        show("snapshot.seq", latestObjectives.seq)
        show("snapshot.level", data.level)
        show("snapshot.objectives", data.objectives and #data.objectives)
        show("snapshot.choice_items", data.rewards and data.rewards.choice and #data.rewards.choice)
        show("snapshot.fixed_items", data.rewards and data.rewards.reward and #data.rewards.reward)
        show("snapshot.currencies", data.rewards and data.rewards.currency and #data.rewards.currency)
        show("snapshot.spell_rewards", data.rewards and data.rewards.spells and #data.rewards.spells)
        show("snapshot.xp", data.xp)
        show("snapshot.money_copper", data.moneyCopper)
        show("snapshot.map_pin", data.mapPin and data.mapPin.mapID)
    else
        print("snapshot.record=unknown")
    end
    if latestLogText then
        show("log_text.seq", latestLogText.seq)
        show("log_text.description_bytes", latestLogText.data.description
            and #latestLogText.data.description)
        show("log_text.objectives_bytes", latestLogText.data.objectives
            and #latestLogText.data.objectives)
    else
        print("log_text.record=unknown")
    end
    if latestReputation then
        show("reputation.seq", latestReputation.seq)
        show("reputation.factions", latestReputation.data.factions
            and #latestReputation.data.factions)
    else
        print("reputation.record=unknown")
    end
end
