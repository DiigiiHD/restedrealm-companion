-- Run with: lua tests/collection_check_smoke.lua addon/RestedRealmCollector/Collector.lua
-- The collection capability check: local only, guarded, and kept across /reload.
local addonFile = assert(arg[1], "pass Collector.lua path")

local frame = {}
function frame:SetScript(_, callback) self.callback = callback end
function frame:RegisterEvent(event)
    -- A client that does not know an event refuses to register it.
    if event == "NEW_TOY_ADDED" then error("unknown event") end
end
CreateFrame = function() return frame end
SlashCmdList = {}
local said = {}
DEFAULT_CHAT_FRAME = { AddMessage = function(_, line) said[#said + 1] = line end }
GetBuildInfo = function() return "1.60.1", "70009" end
GetLocale = function() return "enUS" end
GetServerTime = function() return 1790332912 end
UnitName = function() return "Zoë" end
UnitLevel = function() return 12 end
local guid = "Player-0000-00000001"
UnitGUID = function(unit) if unit == "player" then return guid end end

local timers = {}
C_Timer = { After = function(_, fn) timers[#timers + 1] = fn end }
local function runTimers()
    local pending = timers
    timers = {}
    for _, fn in ipairs(pending) do fn() end
end

local function load()
    assert(loadfile(addonFile))("RestedRealmCollector")
    frame.callback(frame, "ADDON_LOADED", "RestedRealmCollector")
end

-- 1. A client without any collection API: the check still runs and says so.
load()
SlashCmdList.RESTEDREALMCOLLECTOR("collections")
local store = RestedRealmCollectorDB.collectionCheck
local first = store.characters[store.lastSlot]
assert(first.achievements.working == false and first.mounts.working == false and first.toys.working == false)
assert(string.find(said[#said], "achievements not exposed; mounts not exposed; toys not exposed", 1, true))
assert(#RestedRealmCollectorDB.records == 0, "the check never becomes an upload record")

-- 2. A client with all three: IDs, counts and a small sample.
local achievements = {
    [6] = { 6, "Level 10", 10, true, 9, 27, 26, "Reach level 10.", 0, 1, "", false, true, "Zoë", false },
    [7] = { 7, "Account deed", 10, true, 9, 26, 26, "", 0x20000, 1, "", false, false, "Other", false },
}
GetAchievementInfo = function(id)
    local a = achievements[id]
    if a then return unpack(a) end
end
local completedCount = 2
GetNumCompletedAchievements = function() return 400, completedCount end
GetTotalAchievementPoints = function() return 20 end
GetLatestCompletedAchievements = function() return 6, 7 end
local collectedMounts = { [101] = true }
C_MountJournal = {
    GetMountIDs = function() return { 100, 101, 102 } end,
    GetMountInfoByID = function(id)
        return "Mount " .. id, 5000 + id, 1, false, true, 1, false, false, nil, false, collectedMounts[id] == true
    end,
}
local companionMounts = 1
GetNumCompanions = function(kind) if kind == "MOUNT" then return companionMounts end end
local ownedToys = { [9001] = true }
C_ToyBox = {
    GetNumToys = function() return 2 end,
    GetNumLearnedDisplayedToys = function() local n = 0; for _ in pairs(ownedToys) do n = n + 1 end; return n end,
    GetToyFromIndex = function(i) return ({ 9000, 9001 })[i] end,
    GetToyInfo = function(id) return id, "Toy " .. id end,
}
PlayerHasToy = function(id) return ownedToys[id] == true end

SlashCmdList.RESTEDREALMCOLLECTOR("collections")
local check = store.characters[store.lastSlot]
assert(check.achievements.working and check.achievements.completed == 2 and check.achievements.points == 20)
local deed = check.achievements.sample[2]
assert(deed.id == 7 and deed.accountWideFlag == true and deed.wasEarnedByMe == false)
assert(check.achievements.sample[1].wasEarnedByMe == true and check.achievements.sample[1].accountWideFlag == false)
assert(check.mounts.journalTotal == 3 and check.mounts.journalCollected == 1)
assert(check.mounts.sample[1].mountID == 101 and check.mounts.sample[1].spellID == 5101)
assert(check.toys.total == 2 and check.toys.learnedDisplayed == 1 and check.toys.sample[1].itemID == 9001)
assert(store.events.ACHIEVEMENT_EARNED == true and store.events.NEW_TOY_ADDED == false)
assert(#RestedRealmCollectorDB.records == 0)

-- 3. One real unlock of each kind: event arguments, status now and two seconds later.
achievements[8] = { 8, "Tamer", 10, false, nil, nil, nil, "", 0, 1, "", false, false, nil, false }
frame.callback(frame, "ACHIEVEMENT_EARNED", 8, false)
achievements[8][4], achievements[8][13] = true, true
completedCount = 3
collectedMounts[102] = true
companionMounts = 2
frame.callback(frame, "NEW_MOUNT_ADDED", 102)
frame.callback(frame, "COMPANION_LEARNED")
ownedToys[9000] = true
frame.callback(frame, "TOYS_UPDATED", nil, nil, nil)
frame.callback(frame, "TOYS_UPDATED", 9000, true, false)
runTimers()
local unlocks = store.unlocks
assert(#unlocks == 4, "a toy list refresh without an item is not an unlock")
assert(unlocks[1].event == "ACHIEVEMENT_EARNED" and unlocks[1].now.status.completed == false)
assert(unlocks[1].later.status.completed == true and unlocks[1].later.completedAchievements == 3)
assert(unlocks[1].args.alreadyEarned == false)
assert(unlocks[2].category == "mount" and unlocks[2].now.status.isCollected == true)
assert(unlocks[2].countBefore == 1 and unlocks[2].now.companionMounts == 2)
assert(unlocks[3].event == "COMPANION_LEARNED" and unlocks[3].id == nil)
assert(unlocks[4].id == 9000 and unlocks[4].now.status.owned == true and unlocks[4].args.isNew == true)
for _, unlock in ipairs(unlocks) do
    assert(unlock.source == nil and unlock.quest == nil and unlock.loot == nil, "no acquisition source is inferred")
end
assert(#RestedRealmCollectorDB.records == 0)

-- 4. A second character: the first character's owned IDs are looked up again.
guid = "Player-0000-00000002"
collectedMounts[101] = nil
SlashCmdList.RESTEDREALMCOLLECTOR("collections")
local second = store.characters[store.lastSlot]
local sawMount = false
for _, row in ipairs(second.crossCheck) do
    if row.category == "mount" and row.id == 101 then
        sawMount = true
        assert(row.otherCollected == true and row.hereCollected == false)
    end
end
assert(sawMount)

-- 5. Nothing personal: no character name and no raw GUID anywhere in the result.
local function serialize(value, out)
    local kind = type(value)
    if kind == "table" then
        out[#out + 1] = "{"
        for k, v in pairs(value) do
            out[#out + 1] = "["; serialize(k, out); out[#out + 1] = "]="; serialize(v, out); out[#out + 1] = ","
        end
        out[#out + 1] = "}"
    elseif kind == "string" then out[#out + 1] = string.format("%q", value)
    else out[#out + 1] = tostring(value) end
    return out
end
local saved = table.concat(serialize(RestedRealmCollectorDB, {}))
assert(not string.find(saved, "Zoë", 1, true) and not string.find(saved, "Other", 1, true))
assert(not string.find(saved, "Player-0000", 1, true))

-- 6. /reload: the game writes the save and loads the addon again from it.
RestedRealmCollectorDB = assert(loadstring("return " .. saved))()
load()
local reloaded = RestedRealmCollectorDB.collectionCheck
assert(reloaded and #reloaded.unlocks == 4 and reloaded.characters[reloaded.lastSlot].mounts.working)
SlashCmdList.RESTEDREALMCOLLECTOR("clear")
assert(RestedRealmCollectorDB.collectionCheck, "clearing upload records keeps the local check")
print("collection check smoke passed")
