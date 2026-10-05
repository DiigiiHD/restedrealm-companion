-- Run with: lua tests/spell_scan_smoke.lua addon/RestedRealmCollector
-- The background spell check: packs, size limit, combat pause, level-ups,
-- late-loading spells, resuming after /reload, and the trainer timing.
local folder = assert(arg[1], "pass the addon folder")

local frame = {}
function frame:SetScript(_, callback) self.callback = callback end
function frame:RegisterEvent() end
CreateFrame = function() return frame end
SlashCmdList = {}
local said = {}
DEFAULT_CHAT_FRAME = { AddMessage = function(_, line) said[#said + 1] = line end }
GetBuildInfo = function() return "1.60.1", "70205" end
GetLocale = function() return "enUS" end
GetServerTime = function() return 1790332912 end
UnitName = function(unit) if unit == "player" then return "Aerith" end end
UnitRace = function(unit) if unit == "player" then return "Undead", "Scourge" end end
UnitClass = function(unit) if unit == "player" then return "Priest", "PRIEST" end end
local level = 10
UnitLevel = function(unit) if unit == "player" then return level end end
UnitGUID = function(unit) if unit == "player" then return "Player-1-0000000A" end end
local fighting = false
InCombatLockdown = function() return fighting end
GetNumTalentTabs = function() return 3 end
GetTalentTabInfo = function(tab) return "Tree", "icon", ({ 0, 0, 5 })[tab] end

local queue = {}
C_Timer = { After = function(_, fn) queue[#queue + 1] = fn end }
local function runTimers(limit)
    local ran = 0
    while #queue > 0 and ran < (limit or 100000) do
        local fn = table.remove(queue, 1)
        fn()
        ran = ran + 1
    end
    return ran
end

local descriptions = {}
local requested = {}
C_Spell = {
    GetSpellDescription = function(id) return descriptions[id] end,
    RequestLoadSpellData = function(id) requested[id] = true end,
}
local ids = {}
for i = 1, 450 do
    ids[i] = 1000 + i
    descriptions[1000 + i] = "Deals " .. i .. " damage to Aerith's foes."
end

-- Loads the addon the way the game does: the data file first. The list the
-- app would have written is put back afterwards.
local function load(list)
    assert(loadfile(folder .. "/SpellScanList.lua"))()
    if list then RestedRealmSpellScanList = list end
    assert(loadfile(folder .. "/Collector.lua"))("RestedRealmCollector")
    frame.callback(frame, "ADDON_LOADED", "RestedRealmCollector")
end
local function login()
    frame.callback(frame, "PLAYER_ENTERING_WORLD")
end
local function scans(from)
    local found = {}
    for i = from or 1, #RestedRealmCollectorDB.records do
        local entry = RestedRealmCollectorDB.records[i]
        if entry.kind == "spell_scan" then found[#found + 1] = entry end
    end
    return found
end
local function finalOf(list)
    for _, entry in ipairs(list) do if entry.data.complete then return entry end end
end

-- 0. The placeholder file loads and no list means no check.
load()
assert(RestedRealmSpellScanList == nil)
login()
runTimers()
assert(#scans() == 0)

-- 1. A list of 450 spells, packs of 200: 200, 200 and a closing 50.
local scanList = { listVersion = "1.60.1.70205-7", build = "1.60.1.70205", packSize = 200, spells = ids }
load(scanList)
login()
runTimers()
local first = scans()
assert(#first == 3, #first)
assert(#first[1].data.entries == 200 and #first[2].data.entries == 200 and #first[3].data.entries == 50)
local closing = first[3]
assert(closing.data.complete and closing.data.checked == 450 and closing.data.unchanged == 0)
assert(closing.data.listSize == 450 and closing.data.empty == 0 and closing.data.level == 10)
assert(first[1].textSchema == 1 and first[1].data.class == "PRIEST" and first[1].data.race == "Scourge")
assert(first[1].data.entries[1].text == "Deals 1 damage to <name>'s foes.", first[1].data.entries[1].text)
assert(first[1].data.talentPoints[3] == 5 and first[1].data.listVersion == "1.60.1.70205-7")
SlashCmdList.RESTEDREALMCOLLECTOR("status")
assert(string.find(said[#said], "spell check done for level 10", 1, true), said[#said])

-- 2. The same level again (a new login): nothing.
local mark = #RestedRealmCollectorDB.records
load(scanList)
assert(RestedRealmSpellScanList == scanList)
login()
runTimers()
assert(#scans(mark + 1) == 0)

-- 3. A level-up where 10 texts change: only those 10, in one closing pack.
for i = 1, 10 do descriptions[1000 + i] = "Deals " .. (i + 1) .. " damage to Aerith's foes." end
level = 11
frame.callback(frame, "PLAYER_LEVEL_UP")
runTimers()
local levelUp = scans(mark + 1)
assert(#levelUp == 1 and #levelUp[1].data.entries == 10, #levelUp)
assert(levelUp[1].data.complete and levelUp[1].data.checked == 450 and levelUp[1].data.unchanged == 440)

-- 4. Combat pauses the check; it continues where it stopped afterwards.
mark = #RestedRealmCollectorDB.records
RestedRealmSpellScanList.listVersion = "1.60.1.70205-8"
level = 12
frame.callback(frame, "PLAYER_LEVEL_UP")
runTimers(5)
fighting = true
runTimers()
local during = #RestedRealmCollectorDB.records
SlashCmdList.RESTEDREALMCOLLECTOR("status")
assert(string.find(said[#said], "paused in combat", 1, true), said[#said])
fighting = false
frame.callback(frame, "PLAYER_REGEN_ENABLED")
runTimers()
local newList = scans(mark + 1)
assert(#newList == 3 and finalOf(newList).data.checked == 450, "a new list version sends every description again")
assert(during == mark, "nothing was recorded during combat")

-- 5. Long descriptions: packs close by size before 200 entries.
mark = #RestedRealmCollectorDB.records
local long = string.rep("x", 5000)
for i = 1, 450 do descriptions[1000 + i] = long .. i end
RestedRealmSpellScanList.listVersion = "1.60.1.70205-9"
frame.callback(frame, "PLAYER_LEVEL_UP")
runTimers()
for _, pack in ipairs(scans(mark + 1)) do
    local chars = 0
    for _, entry in ipairs(pack.data.entries) do chars = chars + #entry.text end
    assert(chars <= 65000 and #pack.data.entries <= 200, chars)
end

-- 6. Spells that load late are looked at again; ones that never load are counted.
mark = #RestedRealmCollectorDB.records
for i = 1, 450 do descriptions[1000 + i] = "Heals " .. i .. "." end
descriptions[1001], descriptions[1002] = "", nil
C_Spell.RequestLoadSpellData = function(id)
    requested[id] = true
    if id == 1001 then descriptions[1001] = "Heals 1 after loading." end
end
RestedRealmSpellScanList.listVersion = "1.60.1.70205-10"
frame.callback(frame, "PLAYER_LEVEL_UP")
runTimers()
local late = scans(mark + 1)
local done = finalOf(late)
assert(done.data.checked == 449 and done.data.empty == 1, done.data.checked .. " " .. done.data.empty)
local sawLate = false
for _, pack in ipairs(late) do
    for _, entry in ipairs(pack.data.entries) do if entry.spellId == 1001 then sawLate = true end end
end
assert(sawLate and requested[1002])

-- 7. /reload in the middle: the next login continues after the last saved pack.
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
descriptions[1002] = "Heals 2."
mark = #RestedRealmCollectorDB.records
RestedRealmSpellScanList.listVersion = "1.60.1.70205-11"
frame.callback(frame, "PLAYER_LEVEL_UP")
runTimers(10)  -- 250 descriptions: one pack of 200 saved, 50 in memory
assert(#scans(mark + 1) == 1)
queue = {}
local saved = table.concat(serialize(RestedRealmCollectorDB, {}))
RestedRealmCollectorDB = assert(loadstring("return " .. saved))()
load(RestedRealmSpellScanList)
login()
runTimers()
local resumed = scans(mark + 1)
local seen, total = {}, 0
for _, pack in ipairs(resumed) do
    for _, entry in ipairs(pack.data.entries) do
        assert(not seen[entry.spellId], "no description is sent twice")
        seen[entry.spellId] = true
        total = total + 1
    end
end
assert(total == 450 and finalOf(resumed).data.checked == 450, total)

-- 8. Text capture off: no check at all.
mark = #RestedRealmCollectorDB.records
SlashCmdList.RESTEDREALMCOLLECTOR("text off")
RestedRealmSpellScanList.listVersion = "1.60.1.70205-12"
frame.callback(frame, "PLAYER_LEVEL_UP")
runTimers()
assert(#scans(mark + 1) == 0)
SlashCmdList.RESTEDREALMCOLLECTOR("text on")

-- 9. The scan never writes outside its packs, and every pack fits the website's limits.
for _, pack in ipairs(scans()) do
    assert(#pack.data.entries <= 200)
end
print("spell scan smoke passed")
