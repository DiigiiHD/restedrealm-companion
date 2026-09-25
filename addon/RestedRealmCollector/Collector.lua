-- RestedRealm Forever collection probe. No gameplay actions or network access.
local ADDON = ...
local VERSION = "0.1.14"
local IDENTITY_SCHEMA = 2
local MAX_RECORDS = 1500
local MAX_TEXT = 8192
local MAX_ITEMS = 200
local MAX_QUEST_ENTRIES = 100
local MAX_OBJECTIVES = 12
local MAX_SERVICES = 100
local MAX_TRAINER_SERVICES = 250
local MAX_SIGHTINGS = 300
local frame = CreateFrame("Frame")
local db
local record
local questScanPending = false
local lootSlots = {}
local pendingLoot = {}
local nextLootCheck = 0
local lastLootDigest
local collectionScope
local lastGossipNPC, lastGossipAt, lastGossipOptions
local lastGossipPoiDigest
local lastEmptyTrainerKey, lastEmptyTrainerAt
local gossipHooksInstalled = false
local scannerTooltip
local blobEventRegistered = false

local function call(fn, ...)
    if type(fn) ~= "function" then return nil end
    local ok, a, b, c, d, e, f, g, h, i, j, k, l = pcall(fn, ...)
    if ok then return a, b, c, d, e, f, g, h, i, j, k, l end
    return nil
end

local function text(value, limit)
    local ok, result = pcall(function()
        if type(value) == "string" then
            return string.sub(value, 1, limit or MAX_TEXT)
        end
    end)
    if ok then return result end
    return nil
end

local function longText(data, key, value)
    data[key] = text(value)
    local ok, length = pcall(function()
        if type(value) == "string" then return #value end
    end)
    if ok and length and length > MAX_TEXT then
        data.textTruncated = data.textTruncated or {}
        data.textTruncated[key] = true
    end
end

local function number(value)
    local ok, result = pcall(function()
        if type(value) == "number" and value == value then return value end
    end)
    if ok then return result end
    return nil
end

local function message(value)
    if DEFAULT_CHAT_FRAME then
        DEFAULT_CHAT_FRAME:AddMessage("RestedRealm Collector: " .. value)
    end
end

local function entityFromGuid(guid)
    guid = text(guid, 100)
    if not guid then return nil, nil end
    -- Creature-0-server-instance-zone-entityID-spawnUID.
    local kind, id = string.match(guid, "^([^-]+)%-[^-]+%-[^-]+%-[^-]+%-[^-]+%-(%d+)%-")
    if kind == "Creature" or kind == "Vehicle" or kind == "GameObject" then
        return kind, tonumber(id)
    end
    return nil, nil
end

local function tooltipSubtitle(lines)
    if type(lines) ~= "table" then return nil, {} end
    local samples = {}
    for i = 2, math.min(#lines, 6) do
        local line = lines[i]
        local value = type(line) == "table" and line.leftText or nil
        if type(value) == "string" then
            samples[#samples + 1] = text(value, 160)
            local subtitle = string.match(value, "^%s*<([^<>]+)>%s*$")
            if subtitle then return text(subtitle, 160), samples end
        end
    end
    return nil, samples
end

local function subtitleBeforeLevel(samples, level)
    if type(samples) ~= "table" or type(level) ~= "number" then return nil end
    local first, second = samples[1], samples[2]
    if type(first) ~= "string" or type(second) ~= "string" then return nil end
    if #first == 0 or string.find(first, "%d") then return nil end
    if tonumber(string.match(second, "(%d+)")) == level then
        return text(first, 160)
    end
end

local function legacyTooltipSubtitle()
    if not scannerTooltip and type(CreateFrame) == "function" then
        local ok, created = pcall(CreateFrame, "GameTooltip",
            "RestedRealmCollectorScannerTooltip", UIParent, "GameTooltipTemplate")
        if ok then scannerTooltip = created end
    end
    local scanner = scannerTooltip
    if not scanner or type(scanner.SetUnit) ~= "function" then return nil end
    local ok, subtitle, samples = pcall(function()
        local lines = {}
        if type(scanner.SetOwner) == "function" then
            scanner:SetOwner(UIParent or WorldFrame, "ANCHOR_NONE")
        end
        if type(scanner.ClearLines) == "function" then scanner:ClearLines() end
        scanner:SetUnit("npc")
        for i = 2, math.min(number(call(scanner.NumLines, scanner)) or 0, 6) do
            local line = _G["RestedRealmCollectorScannerTooltipTextLeft" .. i]
            local value = line and call(line.GetText, line)
            if type(value) == "string" then
                lines[#lines + 1] = text(value, 160)
                local found = string.match(value, "^%s*<([^<>]+)>%s*$")
                if found then return text(found, 160), lines end
            end
        end
        return nil, lines
    end)
    if ok then return subtitle, samples end
    return nil, {}
end

local function interaction()
    local guid = call(UnitGUID, "npc")
    local source = "npc"
    local name = call(UnitName, "npc")
    if not guid then
        guid = call(UnitGUID, "target")
        name = call(UnitName, "target")
        source = "target_unverified"
    end
    local kind, id = entityFromGuid(guid)
    -- A stale target can be another player. Never persist its name or GUID.
    if not kind or not id then name = nil end
    local result = { kind = kind, id = id, name = text(name, 160), source = source }
    if source == "npc" and kind == "Creature" then
        result.level = number(call(UnitLevel, "npc"))
        result.classification = text(call(UnitClassification, "npc"), 40)
        result.creatureType = text(call(UnitCreatureType, "npc"), 80)
    end
    if source == "npc" and kind == "Creature" and C_TooltipInfo then
        local probe = { structured = {}, private = {} }
        local tooltip = call(C_TooltipInfo.GetUnit, "npc")
        if type(tooltip) == "table" and (not tooltip.guid or tooltip.guid == guid)
            and type(tooltip.lines) == "table" then
            result.subtitle, probe.structured = tooltipSubtitle(tooltip.lines)
            if result.subtitle then result.subtitleSource = "npc_unit_tooltip" end
        end
        result.tooltipProbe = probe
    end
    if source == "npc" and kind == "Creature" and not result.subtitle then
        local privateLines
        result.subtitle, privateLines = legacyTooltipSubtitle()
        if not result.tooltipProbe then
            result.tooltipProbe = { structured = {}, private = {} }
        end
        result.tooltipProbe.private = privateLines or {}
        if result.subtitle then result.subtitleSource = "private_unit_tooltip" end
    end
    if not result.subtitle and result.tooltipProbe then
        result.subtitle = subtitleBeforeLevel(result.tooltipProbe.structured, result.level)
            or subtitleBeforeLevel(result.tooltipProbe.private, result.level)
        if result.subtitle then result.subtitleSource = "tooltip_role_before_level" end
    end
    if result.subtitle then result.tooltipProbe = nil end
    return result
end

local function location()
    local mapID = C_Map and call(C_Map.GetBestMapForUnit, "player") or nil
    local x, y
    if mapID and C_Map and C_Map.GetPlayerMapPosition then
        local position = call(C_Map.GetPlayerMapPosition, mapID, "player")
        if position and position.GetXY then
            x, y = call(position.GetXY, position)
        end
    end
    local instanceName, instanceType, difficultyID = call(GetInstanceInfo)
    return {
        mapID = number(mapID),
        x = number(x), y = number(y),
        zone = text(call(GetZoneText), 100),
        subzone = text(call(GetSubZoneText), 100),
        instanceType = text(instanceType, 32),
        difficultyID = number(difficultyID),
    }
end

local function targetSighting()
    if db.sightingCount >= MAX_SIGHTINGS then return end
    local kind, id = entityFromGuid(call(UnitGUID, "target"))
    if not kind or not id then return end
    local where = location()
    if not where.mapID or not where.x or not where.y then return end
    local gridX = math.floor(where.x * 100)
    local gridY = math.floor(where.y * 100)
    local key = table.concat({ collectionScope or "unknown", kind, tostring(id),
        tostring(where.mapID), tostring(gridX), tostring(gridY) }, ":")
    if db.sightingKeys[key] then return end
    local data = { entity = { kind = kind, id = id,
        name = text(call(UnitName, "target"), 160),
        level = number(call(UnitLevel, "target")),
        classification = text(call(UnitClassification, "target"), 40),
        creatureType = text(call(UnitCreatureType, "target"), 80) },
        positionMeaning = "player_location_while_targeting_entity",
        precision = "one_percent_map_grid" }
    local before = #db.records
    record("entity_sighting", data)
    if #db.records > before then
        db.sightingKeys[key] = true
        db.sightingCount = db.sightingCount + 1
    end
end

local function context()
    local version, build = call(GetBuildInfo)
    local _, classToken = call(UnitClass, "player")
    local _, raceToken = call(UnitRace, "player")
    return {
        product = "wow_classic_beta",
        version = text(version, 40), build = text(build, 20),
        locale = text(call(GetLocale), 12),
        realm = text(call(GetRealmName), 100),
        faction = text(call(UnitFactionGroup, "player"), 32),
        class = text(classToken, 32), race = text(raceToken, 32),
        level = number(call(UnitLevel, "player")),
        location = location(),
    }
end

record = function(kind, data)
    if not db or not db.enabled then return end
    if #db.records >= MAX_RECORDS then
        db.dropped = (db.dropped or 0) + 1
        if not db.fullNotice then
            db.fullNotice = true
            message("local storage is full. Collection paused until data is copied out.")
        end
        return
    end
    db.nextSeq = (db.nextSeq or 0) + 1
    local observedAt = call(GetServerTime) or call(time)
    db.records[#db.records + 1] = {
        seq = db.nextSeq,
        identitySchema = IDENTITY_SCHEMA,
        collectorVersion = VERSION,
        kind = kind,
        observedAt = number(observedAt),
        context = context(),
        data = data,
    }
end

local function itemID(link)
    link = text(link, 500)
    if not link then return nil end
    return tonumber(string.match(link, "item:(%d+)"))
end

local function itemVariant(link)
    if type(link) ~= "string" then return nil end
    local value = string.match(link, "item:(%d+[:%d%-]*)")
    return text(value, 200)
end

local function recentGossipNPC()
    if not lastGossipNPC then return nil end
    local now = number(call(GetServerTime))
    if now and lastGossipAt and now - lastGossipAt > 30 then return nil end
    return lastGossipNPC
end

local function gossipPoi()
    local api = C_GossipInfo
    local npc = recentGossipNPC()
    if not api or not npc then return end
    local mapID = location().mapID
    if not mapID then return end
    local poiID = number(call(api.GetPoiForUiMapID, mapID))
    if not poiID or poiID <= 0 then return end
    local info = call(api.GetPoiInfo, mapID, poiID)
    if type(info) ~= "table" then return end
    local x, y
    if type(info.position) == "table" then
        x, y = call(info.position.GetXY, info.position)
        x, y = number(x) or number(info.position.x),
            number(y) or number(info.position.y)
    end
    if not x or not y or x < 0 or x > 1 or y < 0 or y > 1 then return end
    local data = { npc = npc, mapID = mapID, poiID = poiID,
        name = text(info.name, 160), textureIndex = number(info.textureIndex),
        x = x, y = y, inBattleMap = info.inBattleMap,
        positionMeaning = "client_gossip_poi" }
    local digest = table.concat({ tostring(npc.id), tostring(mapID),
        tostring(poiID), tostring(x), tostring(y) }, ":")
    if lastGossipPoiDigest ~= digest then
        local before = #db.records
        record("gossip_poi", data)
        if #db.records > before then lastGossipPoiDigest = digest end
    end
end

local function installGossipHooks()
    if gossipHooksInstalled or not C_GossipInfo or type(hooksecurefunc) ~= "function" then return end
    local installed = false
    for _, method in ipairs({ "SelectOption", "SelectOptionByIndex" }) do
        if type(C_GossipInfo[method]) == "function" then
            local methodName = method
            local ok = pcall(hooksecurefunc, C_GossipInfo, methodName, function(argument)
                local handled = pcall(function()
                    if not db or not db.enabled then return end
                    local npc = recentGossipNPC()
                    if not npc then return end
                    local raw = number(argument)
                    local selected
                    for _, option in ipairs(lastGossipOptions or {}) do
                        if (methodName == "SelectOptionByIndex" and option.orderIndex == raw)
                            or (methodName == "SelectOption" and option.id == raw) then
                            selected = option
                            break
                        end
                    end
                    record("gossip_selection", {
                        npc = npc, selectionAPI = methodName,
                        rawSelectionArgument = raw,
                        selectionArgumentKind = methodName == "SelectOptionByIndex"
                            and "order_index_zero_based" or "gossip_option_id",
                        optionID = selected and selected.id
                            or (methodName == "SelectOption" and raw or nil),
                        optionName = selected and selected.name,
                        optionOrderIndex = selected and selected.orderIndex,
                        meaning = "gossip_selection_api_called",
                    })
                end)
                if not handled and db then db.errors = (db.errors or 0) + 1 end
            end)
            if ok then installed = true end
        end
    end
    gossipHooksInstalled = installed
end

local function questKey(id)
    return (collectionScope or "unknown_build:unknown_locale") .. ":" .. tostring(id)
end

local function gossip()
    installGossipHooks()
    local api = C_GossipInfo
    if not api then return end
    local data = { npc = interaction(), options = {}, availableQuests = {}, activeQuests = {} }
    lastGossipNPC = data.npc
    lastGossipAt = number(call(GetServerTime))
    if db.captureText then longText(data, "dialogue", call(api.GetText)) end
    local options = call(api.GetOptions)
    if type(options) == "table" then
        for i = 1, math.min(#options, 40) do
            local option = options[i]
            if type(option) == "table" then
                data.options[#data.options + 1] = {
                    id = number(option.gossipOptionID),
                    name = text(option.name, 300),
                    status = number(option.status),
                    orderIndex = number(option.orderIndex),
                    spellID = number(option.spellID),
                    flags = number(option.flags),
                    failureDescription = text(option.failureDescription, 300),
                }
            end
        end
    end
    lastGossipOptions = data.options
    for _, pair in ipairs({ { "GetAvailableQuests", data.availableQuests }, { "GetActiveQuests", data.activeQuests } }) do
        local quests = call(api[pair[1]])
        if type(quests) == "table" then
            for i = 1, math.min(#quests, 40) do
                local quest = quests[i]
                if type(quest) == "table" then
                    pair[2][#pair[2] + 1] = {
                        id = number(quest.questID),
                        title = text(quest.title, 300),
                        level = number(quest.questLevel),
                        complete = quest.isComplete == true,
                        trivial = quest.isTrivial,
                        frequency = number(quest.frequency),
                        repeatable = quest.repeatable,
                        legendary = quest.isLegendary,
                        ignored = quest.isIgnored,
                        important = quest.isImportant,
                        meta = quest.isMeta,
                    }
                end
            end
        end
    end
    record("gossip", data)
    gossipPoi()
end

local function questGreeting()
    local data = { npc = interaction(), available = {}, active = {} }
    if db.captureText then longText(data, "greeting", call(GetGreetingText)) end
    local available = number(call(GetNumAvailableQuests)) or 0
    for i = 1, math.min(available, 40) do
        local trivial, frequency, repeatable, legendary, questID = call(GetAvailableQuestInfo, i)
        data.available[#data.available + 1] = {
            id = number(questID), title = text(call(GetAvailableTitle, i), 300),
            level = number(call(GetAvailableLevel, i)),
            trivial = trivial, frequency = number(frequency),
            repeatable = repeatable, legendary = legendary,
        }
    end
    local active = number(call(GetNumActiveQuests)) or 0
    for i = 1, math.min(active, 40) do
        local title, complete = call(GetActiveTitle, i)
        data.active[#data.active + 1] = {
            id = number(call(GetActiveQuestID, i)),
            title = text(title, 300), level = number(call(GetActiveLevel, i)),
            complete = complete == true,
        }
    end
    record("quest_greeting", data)
end

local function merchant()
    local count = number(call(GetMerchantNumItems)) or 0
    local data = { npc = interaction(), items = {}, shownCount = count }
    for i = 1, math.min(count, MAX_ITEMS) do
        local info = C_MerchantFrame and call(C_MerchantFrame.GetItemInfo, i)
        local name, price, stackCount, available, purchasable, usable, extendedCost
        if type(info) == "table" then
            name, price = info.name, info.price
            stackCount, available = info.stackCount, info.numAvailable
            purchasable, usable = info.isPurchasable, info.isUsable
            extendedCost = info.hasExtendedCost
        else
            local _, texture
            name, texture, price, stackCount, available, purchasable, usable, extendedCost = call(GetMerchantItemInfo, i)
        end
        local link = call(GetMerchantItemLink, i)
        local id = number(call(GetMerchantItemID, i)) or itemID(link)
        local item = {
            id = id, name = text(name, 300), priceCopper = number(price),
            variant = itemVariant(link),
            stackCount = number(stackCount), available = number(available),
            purchasable = purchasable, usable = usable, extendedCost = extendedCost,
        }
        if extendedCost == true then
            item.otherCosts = {}
            local costCount = number(call(GetMerchantItemCostInfo, i)) or 0
            for costIndex = 1, math.min(costCount, 6) do
                local _, amount, link = call(GetMerchantItemCostItem, i, costIndex)
                item.otherCosts[#item.otherCosts + 1] = {
                    itemID = itemID(link),
                    currencyID = type(link) == "string" and number(tonumber(string.match(link, "currency:(%d+)"))) or nil,
                    amount = number(amount),
                }
            end
        end
        data.items[#data.items + 1] = item
    end
    record("merchant", data)
end

local function trainer()
    local count = number(call(GetNumTrainerServices))
    if not count or count <= 0 then
        local npc = interaction()
        local key = tostring(npc.id) .. ":" .. tostring(count)
        local now = number(call(GetServerTime))
        if key ~= lastEmptyTrainerKey or not now or not lastEmptyTrainerAt
            or now - lastEmptyTrainerAt > 30 then
            record("trainer_window", { npc = npc, shownCount = count,
                serviceCountAPI = type(GetNumTrainerServices) == "function",
                meaning = "trainer_event_without_visible_services" })
            lastEmptyTrainerKey, lastEmptyTrainerAt = key, now
        end
        return
    end
    local data = { npc = interaction(), shownCount = count,
        truncated = count > MAX_TRAINER_SERVICES, services = {} }
    if db.captureText then longText(data, "greeting", call(GetTrainerGreetingText)) end
    for i = 1, math.min(count, MAX_TRAINER_SERVICES) do
        local name, status = call(GetTrainerServiceInfo, i)
        local skillName, skillRequired = call(GetTrainerServiceSkillReq, i)
        data.services[#data.services + 1] = {
            name = text(name, 300), status = text(status, 30),
            priceCopper = number(call(GetTrainerServiceCost, i)),
            levelRequired = number(call(GetTrainerServiceLevelReq, i)),
            skillName = text(skillName, 100), skillRequired = number(skillRequired),
            itemID = itemID(call(GetTrainerServiceItemLink, i)),
        }
    end
    record("trainer", data)
end

local function professionOpened()
    local api = C_TradeSkillUI
    local base = api and call(api.GetBaseProfessionInfo)
    local child = api and call(api.GetChildProfessionInfo)
    local data = { source = "trade_skill_ui" }
    if type(base) == "table" then
        data.professionID = number(base.professionID)
        data.professionName = text(base.professionName, 160)
    end
    if type(child) == "table" then
        data.skillLineID = number(child.professionID)
        data.skillLineName = text(child.professionName, 160)
        data.skillLevel = number(child.skillLevel)
        data.maxSkillLevel = number(child.maxSkillLevel)
    end
    if data.professionID or data.skillLineID then
        record("profession_opened", data)
        return
    end
    -- Classic-style profession windows expose a different, index-based API.
    local name, rank, maxRank = call(GetTradeSkillLine)
    local count = number(call(GetNumTradeSkills))
    if type(name) ~= "string" or not count or count <= 0 then return end
    data = { source = "legacy_trade_skill", professionName = text(name, 160),
        skillLevel = number(rank), maxSkillLevel = number(maxRank),
        shownCount = count, truncated = count > MAX_ITEMS, recipes = {} }
    local fingerprint = { data.professionName, tostring(data.skillLevel),
        tostring(data.maxSkillLevel), tostring(count) }
    for i = 1, math.min(count, MAX_ITEMS) do
        local recipeName, recipeType = call(GetTradeSkillInfo, i)
        if recipeType ~= "header" and type(recipeName) == "string" then
            local outputLink = call(GetTradeSkillItemLink, i)
            local recipeLink = call(GetTradeSkillRecipeLink, i)
            local minimum, maximum = call(GetTradeSkillNumMade, i)
            local recipeSpellID
            if type(recipeLink) == "string" then
                recipeSpellID = tonumber(string.match(recipeLink, "enchant:(%d+)"))
                    or tonumber(string.match(recipeLink, "spell:(%d+)"))
            end
            local row = { index = i, name = text(recipeName, 300),
                difficulty = text(recipeType, 30),
                outputItemID = itemID(outputLink),
                outputVariant = itemVariant(outputLink),
                recipeSpellID = recipeSpellID,
                quantityMin = number(minimum), quantityMax = number(maximum),
                reagents = {} }
            local reagentCount = number(call(GetTradeSkillNumReagents, i)) or 0
            for j = 1, math.min(reagentCount, 20) do
                local reagentName, _, required = call(GetTradeSkillReagentInfo, i, j)
                local reagentLink = call(GetTradeSkillReagentItemLink, i, j)
                row.reagents[#row.reagents + 1] = {
                    id = itemID(reagentLink), name = text(reagentName, 160),
                    required = number(required),
                }
            end
            data.recipes[#data.recipes + 1] = row
            fingerprint[#fingerprint + 1] = table.concat({ tostring(i),
                row.name, tostring(row.difficulty), tostring(row.outputItemID),
                tostring(row.recipeSpellID), tostring(row.quantityMin),
                tostring(row.quantityMax) }, ":")
            for _, reagent in ipairs(row.reagents) do
                fingerprint[#fingerprint + 1] = tostring(reagent.id)
                    .. ":" .. tostring(reagent.required)
            end
        end
    end
    local key = (collectionScope or "unknown") .. ":profession:" .. data.professionName
    local digest = table.concat(fingerprint, "|")
    if db.professionFingerprints[key] ~= digest then
        local before = #db.records
        record("profession_catalog", data)
        if #db.records > before then db.professionFingerprints[key] = digest end
    end
end

local function recipeObservation(recipeID, event)
    recipeID = number(recipeID)
    local api = C_TradeSkillUI
    if not recipeID or not api then return end
    local info = call(api.GetRecipeInfo, recipeID)
    local data = { id = recipeID, event = event }
    if type(info) == "table" then
        data.name = text(info.name, 300)
        data.learned = info.learned
        data.categoryID = number(info.categoryID)
        data.difficulty = number(info.relativeDifficulty)
        data.skillLineAbilityID = number(info.skillLineAbilityID)
    end
    local skillLineID, skillLineName, parentID = call(api.GetTradeSkillLineForRecipe, recipeID)
    data.skillLineID = number(skillLineID)
    data.skillLineName = text(skillLineName, 160)
    data.parentSkillLineID = number(parentID)
    local schematic = call(api.GetRecipeSchematic, recipeID, false)
    if type(schematic) == "table" then
        data.outputItemID = number(schematic.outputItemID)
        data.quantityMin = number(schematic.quantityMin)
        data.quantityMax = number(schematic.quantityMax)
        data.reagents = {}
        if type(schematic.reagentSlotSchematics) == "table" then
            for i = 1, math.min(#schematic.reagentSlotSchematics, 20) do
                local slot = schematic.reagentSlotSchematics[i]
                if type(slot) == "table" then
                    local entry = { quantityRequired = number(slot.quantityRequired),
                        required = slot.required, alternatives = {} }
                    if type(slot.reagents) == "table" then
                        for j = 1, math.min(#slot.reagents, 6) do
                            local reagent = slot.reagents[j]
                            if type(reagent) == "table" then
                                entry.alternatives[#entry.alternatives + 1] = {
                                    itemID = number(reagent.itemID),
                                    currencyID = number(reagent.currencyID),
                                }
                            end
                        end
                    end
                    data.reagents[#data.reagents + 1] = entry
                end
            end
        end
    end
    if event == "learned" then record("recipe", data); return end
    local key = questKey(recipeID)
    local digest = tostring(data.outputItemID) .. ":" .. tostring(data.name)
    for _, slot in ipairs(data.reagents or {}) do
        digest = digest .. ":" .. tostring(slot.quantityRequired)
        for _, reagent in ipairs(slot.alternatives) do
            digest = digest .. ":" .. tostring(reagent.itemID)
                .. ":" .. tostring(reagent.currencyID)
        end
    end
    if db.recipeFingerprints[key] ~= digest then
        local before = #db.records
        record("recipe", data)
        if #db.records > before then db.recipeFingerprints[key] = digest end
    end
end

local function taxi()
    local count = number(call(NumTaxiNodes)) or 0
    if count <= 0 then return end
    local data = { npc = interaction(), shownCount = count, nodes = {},
        coordinateSystem = "taxi_map" }
    for i = 1, math.min(count, MAX_SERVICES) do
        local x, y = call(TaxiNodePosition, i)
        data.nodes[#data.nodes + 1] = {
            name = text(call(TaxiNodeName, i), 160),
            status = text(call(TaxiNodeGetType, i), 30),
            x = number(x), y = number(y),
            costCopper = number(call(TaxiNodeCost, i)),
        }
    end
    record("taxi_map", data)
end

local function quest(stage, startItemID)
    local questID = number(call(GetQuestID))
    local level = questID and number(call(GetQuestDifficultyLevel, questID)) or nil
    if not level and questID and C_QuestLog then
        level = number(call(C_QuestLog.GetQuestDifficultyLevel, questID))
    end
    local data = {
        stage = stage, npc = interaction(), id = questID, level = level,
        title = text(call(GetTitleText), 300),
        startItemID = number(startItemID),
    }
    if db.captureText then
        longText(data, "questText", call(GetQuestText))
        longText(data, "objectiveText", call(GetObjectiveText))
        longText(data, "progressText", call(GetProgressText))
        longText(data, "rewardText", call(GetRewardText))
    end
    data.items = {}
    for _, rewardType in ipairs({ "required", "reward", "choice" }) do
            local count
            if rewardType == "required" then count = number(call(GetNumQuestItems))
            elseif rewardType == "reward" then count = number(call(GetNumQuestRewards))
            else count = number(call(GetNumQuestChoices)) end
            for i = 1, math.min(count or 0, 40) do
                local name, _, quantity, quality, usable, directID = call(GetQuestItemInfo, rewardType, i)
                local link = call(GetQuestItemLink, rewardType, i)
                local id = number(directID) or itemID(link)
                data.items[#data.items + 1] = {
                    type = rewardType, id = id, name = text(name, 300),
                    variant = itemVariant(link),
                    quantity = number(quantity), quality = number(quality),
                    usable = usable,
                }
            end
    end
    data.moneyCopper = number(call(GetRewardMoney))
    data.xp = number(call(GetRewardXP))
    data.honor = number(call(GetRewardHonor))
    local rewardTitle = call(GetRewardTitle)
    data.titleRewardID = number(rewardTitle)
    if not data.titleRewardID then data.titleRewardName = text(rewardTitle, 160) end
    data.currencies = {}
    data.spellRewardIDs = {}
    local dialogSpells = questID and C_QuestInfoSystem
        and call(C_QuestInfoSystem.GetQuestRewardSpells, questID)
    if type(dialogSpells) == "table" then
        for i = 1, math.min(#dialogSpells, 40) do
            local spellID = number(dialogSpells[i])
            if spellID then data.spellRewardIDs[#data.spellRewardIDs + 1] = spellID end
        end
    end
    local currencyCount = number(call(GetNumRewardCurrencies)) or 0
    for i = 1, math.min(currencyCount, 40) do
        local name, _, quantity, currencyID = call(GetQuestCurrencyInfo, "reward", i)
        data.currencies[#data.currencies + 1] = {
            id = number(currencyID), name = text(name, 160), quantity = number(quantity),
            source = "quest_dialog",
        }
    end
    if questID then
        local logMoney = number(call(GetQuestLogRewardMoney, questID))
        local logXP = number(call(GetQuestLogRewardXP, questID))
        if (not data.moneyCopper or data.moneyCopper == 0) and logMoney then data.moneyCopper = logMoney end
        if (not data.xp or data.xp == 0) and logXP then data.xp = logXP end
    end
    record("quest", data)
end

local function questLogRewards(id)
    local result = { choice = {}, reward = {}, currency = {}, spells = {} }
    for _, pair in ipairs({
        { "choice", GetNumQuestLogChoices, GetQuestLogChoiceInfo },
        { "reward", GetNumQuestLogRewards, GetQuestLogRewardInfo },
    }) do
        local count = number(call(pair[2], id)) or 0
        for i = 1, math.min(count, 40) do
            local name, _, quantity, quality, usable, directID, itemLevel = call(pair[3], i, id)
            local link = call(GetQuestLogItemLink, pair[1], i, id)
            result[pair[1]][#result[pair[1]] + 1] = {
                id = number(directID) or itemID(link), name = text(name, 300),
                variant = itemVariant(link),
                quantity = number(quantity), quality = number(quality), usable = usable,
                itemLevel = number(itemLevel),
            }
        end
    end
    local api = C_QuestLog or {}
    local currencies = call(api.GetQuestRewardCurrencies, id)
    if type(currencies) == "table" then
        for i = 1, math.min(#currencies, 40) do
            local currency = currencies[i]
            if type(currency) == "table" then
                result.currency[#result.currency + 1] = {
                    id = number(currency.currencyID), name = text(currency.name, 160),
                    quantity = number(currency.totalRewardAmount),
                    baseAmount = number(currency.baseRewardAmount),
                    bonusAmount = number(currency.bonusRewardAmount),
                    source = "C_QuestLog.GetQuestRewardCurrencies",
                }
            end
        end
    else
        local count = number(call(GetNumQuestLogRewardCurrencies, id)) or 0
        for i = 1, math.min(count, 40) do
            local name, _, quantity, currencyID = call(GetQuestLogRewardCurrencyInfo, i, id)
            result.currency[#result.currency + 1] = {
                id = number(currencyID), name = text(name, 160), quantity = number(quantity),
                source = "GetQuestLogRewardCurrencyInfo",
            }
        end
    end
    local spells = C_QuestInfoSystem and call(C_QuestInfoSystem.GetQuestRewardSpells, id)
    if type(spells) == "table" then
        for i = 1, math.min(#spells, 40) do
            local spellID = number(spells[i])
            if spellID then result.spells[#result.spells + 1] = spellID end
        end
    end
    result.honor = number(call(GetQuestLogRewardHonor, id))
    return result
end

local function loot()
    lootSlots = {}
    local count = number(call(GetNumLootItems)) or 0
    local data = { items = {}, shownCount = count }
    for i = 1, math.min(count, 40) do
        local _, name, quantity, currencyID, quality, locked,
            isQuestItem, questID, isActive, isCoin = call(GetLootSlotInfo, i)
        local link = call(GetLootSlotLink, i)
        local id = itemID(link)
        local sources = {}
        if type(GetLootSourceInfo) == "function" then
            local ok, values = pcall(function() return { GetLootSourceInfo(i) } end)
            if ok and type(values) == "table" then
                for j = 1, math.min(#values, 12), 2 do
                    local sourceKind, sourceID = entityFromGuid(values[j])
                    if sourceID then
                        sources[#sources + 1] = { kind = sourceKind, id = sourceID,
                            quantity = number(values[j + 1]) }
                    end
                end
            end
        end
        local firstSource = sources[1]
        if id or number(currencyID) or firstSource then
            local item = {
                id = id, name = text(name, 300), quantity = number(quantity),
                variant = itemVariant(link),
                currencyID = number(currencyID), quality = number(quality),
                locked = locked, isQuestItem = isQuestItem,
                questID = number(questID), isActive = isActive,
                isCoin = isCoin,
                sourceKind = firstSource and firstSource.kind,
                sourceID = firstSource and firstSource.id,
                sources = sources,
            }
            data.items[#data.items + 1] = item
            lootSlots[i] = { item = item,
                countBefore = id and number(call(GetItemCount, id, false, false)) or nil }
        end
    end
    if #data.items > 0 then
        local parts = {}
        for _, item in ipairs(data.items) do
            parts[#parts + 1] = table.concat({ tostring(item.id), tostring(item.currencyID),
                tostring(item.quantity), tostring(item.questID),
                tostring(item.sourceID) }, ":")
        end
        local digest = table.concat(parts, "|")
        if digest ~= lastLootDigest then
            local before = #db.records
            record("loot_window", data)
            if #db.records > before then lastLootDigest = digest end
        end
    end
end

local function checkPendingLoot()
    for key, pending in pairs(pendingLoot) do
        local after = number(call(GetItemCount, pending.itemID, false, false))
        if after and after > pending.before then
            record("item_count_increase", {
                itemID = pending.itemID, quantityDelta = after - pending.before,
                candidateSourceID = pending.sourceID,
                candidateQuestID = pending.questID,
                lootSlotSequence = pending.slotSequence,
                meaning = "bag_count_increased_after_loot_slot_cleared",
            })
            pendingLoot[key] = nil
        end
    end
end

local function queueLootCheck(item, before, slotSequence)
    if not item.id or before == nil then return end
    nextLootCheck = nextLootCheck + 1
    local key = nextLootCheck
    pendingLoot[key] = { itemID = item.id, before = before,
        sourceID = item.sourceID, questID = item.questID,
        slotSequence = slotSequence }
    if C_Timer and type(C_Timer.After) == "function" then
        for _, delay in ipairs({ 0.5, 2, 5 }) do
            C_Timer.After(delay, function()
                if pendingLoot[key] then checkPendingLoot() end
                if delay == 5 then pendingLoot[key] = nil end
            end)
        end
    else
        checkPendingLoot()
        pendingLoot[key] = nil
    end
end

local function questLogEntry(index)
    local api = C_QuestLog
    if api then
        local info = call(api.GetInfo, index)
        if type(info) == "table" then
            local id = number(info.questID)
            if id and id > 0 then
                return id, text(info.title, 300), number(info.level), {
                    frequency = number(info.frequency),
                    suggestedGroup = number(info.suggestedGroup),
                    isComplete = info.isComplete,
                    isOnMap = info.isOnMap,
                    hasLocalPOI = info.hasLocalPOI,
                    isTask = info.isTask,
                    isBounty = info.isBounty,
                    isStory = info.isStory,
                    isScaling = info.isScaling,
                }
            end
        end
        local id = number(call(api.GetQuestIDForLogIndex, index))
        if id and id > 0 then
            return id, text(call(api.GetTitleForLogIndex, index), 300),
                number(call(api.GetQuestDifficultyLevel, id))
        end
    end
    if type(GetQuestLogTitle) ~= "function" then return nil end
    local ok, title, level, _, fourth, fifth, sixth, seventh, eighth, ninth = pcall(GetQuestLogTitle, index)
    if not ok then return nil end
    -- Forever's legacy quest log can return questID in position 8 or 9.
    local id = number(eighth)
    if not id or id <= 0 then id = number(ninth) end
    if not id or id <= 0 then return nil end
    return id, text(title, 300), number(level)
end

local function questMapPin(id)
    local api = C_QuestLog
    if not api or type(api.GetMapForQuestPOIs) ~= "function" then return nil end
    local mapID = number(call(api.GetMapForQuestPOIs))
    if not mapID or mapID <= 0 then return nil end
    local completed, x, y, objectiveIndex = call(QuestPOIGetIconInfo, id)
    x, y = number(x), number(y)
    local source = "QuestPOIGetIconInfo"
    if (not x or not y) and type(api.GetQuestsOnMap) == "function" then
        local quests = call(api.GetQuestsOnMap, mapID)
        if type(quests) == "table" then
            for i = 1, math.min(#quests, MAX_QUEST_ENTRIES) do
                local entry = quests[i]
                if type(entry) == "table" and number(entry.questID) == id then
                    x, y = number(entry.x), number(entry.y)
                    completed, objectiveIndex = nil, nil
                    source = "C_QuestLog.GetQuestsOnMap"
                    break
                end
            end
        end
    end
    if not x or not y or x < 0 or x > 1 or y < 0 or y > 1 then return nil end
    return { mapID = mapID, x = x, y = y, source = source,
        objectiveIndex = number(objectiveIndex), completed = completed,
        geometry = "pin_not_area" }
end

local function scanSelectedQuestReputation()
    if type(GetNumQuestLogRewardFactions) ~= "function"
        or type(GetQuestLogRewardFactionInfo) ~= "function" then return end
    local index = number(call(GetQuestLogSelection))
    if not index or index < 1 then return end
    local id = questLogEntry(index)
    if not id then return end
    local count = number(call(GetNumQuestLogRewardFactions)) or 0
    if count <= 0 then return end
    local data = { id = id, source = "selected_quest_log", factions = {} }
    local fingerprint = { tostring(id) }
    for i = 1, math.min(count, 20) do
        local factionID, rawAmount = call(GetQuestLogRewardFactionInfo, i)
        factionID, rawAmount = number(factionID), number(rawAmount)
        if factionID then
            data.factions[#data.factions + 1] = {
                id = factionID, rawAmount = rawAmount,
                amountScale = "unverified",
            }
            fingerprint[#fingerprint + 1] = tostring(factionID) .. ":" .. tostring(rawAmount)
        end
    end
    if #data.factions == 0 then return end
    local digest = table.concat(fingerprint, "|")
    local key = questKey(id)
    if db.questRepFingerprints[key] ~= digest then
        local before = #db.records
        record("quest_reputation", data)
        if #db.records > before then db.questRepFingerprints[key] = digest end
    end
end

local function scanQuestLogText(count)
    if not db.captureText or type(GetQuestLogQuestText) ~= "function" then return end
    for index = 1, math.min(count, MAX_QUEST_ENTRIES) do
        local id = questLogEntry(index)
        if id then
            local description, objectives = call(GetQuestLogQuestText, index)
            if (type(description) == "string" and #description > 0)
                or (type(objectives) == "string" and #objectives > 0) then
                local data = { id = id, source = "GetQuestLogQuestText",
                    questLogIndex = index }
                longText(data, "description", description)
                longText(data, "objectives", objectives)
                local digest = tostring(data.description) .. "|" .. tostring(data.objectives)
                local key = questKey(id)
                if db.questTextFingerprints[key] ~= digest then
                    local before = #db.records
                    record("quest_log_text", data)
                    if #db.records > before then db.questTextFingerprints[key] = digest end
                end
            end
        end
    end
end

local function scanQuestObjectives()
    if not db or not db.enabled then return end
    local api = C_QuestLog or {}
    local count = number(call(api.GetNumQuestLogEntries))
        or number(call(GetNumQuestLogEntries)) or 0
    db.apiStatus = {
        questEntries = count,
        questInfo = type(api.GetInfo) == "function",
        questIDForIndex = type(api.GetQuestIDForLogIndex) == "function",
        legacyQuestTitle = type(GetQuestLogTitle) == "function",
        questObjectives = type(api.GetQuestObjectives) == "function",
        questLogText = type(GetQuestLogQuestText) == "function",
        questPOI = type(QuestPOIGetIconInfo) == "function",
        questLogChoices = type(GetNumQuestLogChoices) == "function",
        questLogChoiceInfo = type(GetQuestLogChoiceInfo) == "function",
        questLogRewardXP = type(GetQuestLogRewardXP) == "function",
        questLogRewardMoney = type(GetQuestLogRewardMoney) == "function",
        questLogRewardCurrencies = type(api.GetQuestRewardCurrencies) == "function",
        legacyQuestLogCurrencies = type(GetNumQuestLogRewardCurrencies) == "function",
        questRewardSpells = C_QuestInfoSystem and type(C_QuestInfoSystem.GetQuestRewardSpells) == "function" or false,
        questLogReputation = type(GetNumQuestLogRewardFactions) == "function"
            and type(GetQuestLogRewardFactionInfo) == "function",
        questDialogCurrencies = type(GetNumRewardCurrencies) == "function",
        tooltipUnit = C_TooltipInfo and type(C_TooltipInfo.GetUnit) == "function" or false,
        greetingAvailableInfo = type(GetAvailableQuestInfo) == "function",
        greetingAvailableLevel = type(GetAvailableLevel) == "function",
        greetingActiveID = type(GetActiveQuestID) == "function",
        greetingActiveLevel = type(GetActiveLevel) == "function",
        questsOnMap = type(api.GetQuestsOnMap) == "function",
        questBlobCount = type(GetQuestPOIBlobCount) == "function",
        questBlobMembership = C_Minimap and type(C_Minimap.IsInsideQuestBlob) == "function" or false,
        questBlobEdgeEvent = blobEventRegistered,
        questUiMapID = type(GetQuestUiMapID) == "function",
        gossipSelectionHook = gossipHooksInstalled,
        gossipPoi = C_GossipInfo and type(C_GossipInfo.GetPoiForUiMapID) == "function"
            and type(C_GossipInfo.GetPoiInfo) == "function" or false,
        recipeInfo = C_TradeSkillUI and type(C_TradeSkillUI.GetRecipeInfo) == "function" or false,
        recipeSchematic = C_TradeSkillUI and type(C_TradeSkillUI.GetRecipeSchematic) == "function" or false,
        selectedRecipeID = C_TradeSkillUI and type(C_TradeSkillUI.GetSelectedRecipeID) == "function" or false,
        legacyTradeSkills = type(GetTradeSkillLine) == "function"
            and type(GetNumTradeSkills) == "function"
            and type(GetTradeSkillInfo) == "function",
        trainerServices = type(GetNumTrainerServices) == "function",
    }
    scanSelectedQuestReputation()
    scanQuestLogText(count)
    if type(api.GetQuestObjectives) ~= "function" then return end
    for index = 1, math.min(count, MAX_QUEST_ENTRIES) do
        local id, title, level, questMeta = questLogEntry(index)
        if id then
            local objectives = call(api.GetQuestObjectives, id)
            if type(objectives) == "table" then
                local onMap, hasLocalPOI = call(api.IsOnMap, id)
                local data = { id = id, title = title, level = level,
                    questMeta = questMeta,
                    objectives = {}, mapPin = questMapPin(id),
                    mapHints = {
                        questUiMapID = number(call(GetQuestUiMapID, id)),
                        blobCount = number(call(GetQuestPOIBlobCount, id)),
                        playerInsideBlob = C_Minimap and call(C_Minimap.IsInsideQuestBlob, id),
                        onMap = onMap, hasLocalPOI = hasLocalPOI,
                        geometry = "none",
                    },
                    xp = number(call(GetQuestLogRewardXP, id)),
                    moneyCopper = number(call(GetQuestLogRewardMoney, id)),
                    rewards = questLogRewards(id) }
                local fingerprint = { tostring(id), title or "" }
                fingerprint[#fingerprint + 1] = tostring(level)
                fingerprint[#fingerprint + 1] = tostring(data.xp)
                fingerprint[#fingerprint + 1] = tostring(data.moneyCopper)
                fingerprint[#fingerprint + 1] = table.concat({
                    tostring(data.mapHints.questUiMapID),
                    tostring(data.mapHints.blobCount),
                    tostring(data.mapHints.playerInsideBlob),
                    tostring(data.mapHints.onMap),
                    tostring(data.mapHints.hasLocalPOI),
                }, ":")
                if questMeta then
                    for _, field in ipairs({ "frequency", "suggestedGroup", "isComplete",
                        "isOnMap", "hasLocalPOI", "isTask", "isBounty", "isStory", "isScaling" }) do
                        fingerprint[#fingerprint + 1] = field .. ":" .. tostring(questMeta[field])
                    end
                end
                fingerprint[#fingerprint + 1] = tostring(data.rewards.honor)
                for _, currency in ipairs(data.rewards.currency) do
                    fingerprint[#fingerprint + 1] = table.concat({
                        "currency", tostring(currency.id), tostring(currency.name),
                        tostring(currency.quantity), tostring(currency.baseAmount),
                        tostring(currency.bonusAmount),
                    }, ":")
                end
                for _, spellID in ipairs(data.rewards.spells) do
                    fingerprint[#fingerprint + 1] = "spell:" .. tostring(spellID)
                end
                for _, rewardType in ipairs({ "choice", "reward" }) do
                    for _, reward in ipairs(data.rewards[rewardType]) do
                        fingerprint[#fingerprint + 1] = table.concat({
                            rewardType, tostring(reward.id), tostring(reward.name),
                            tostring(reward.quantity), tostring(reward.quality),
                            tostring(reward.usable), tostring(reward.itemLevel),
                            tostring(reward.variant),
                        }, ":")
                    end
                end
                if data.mapPin then
                    fingerprint[#fingerprint + 1] = table.concat({
                        tostring(data.mapPin.mapID), tostring(data.mapPin.x),
                        tostring(data.mapPin.y), tostring(data.mapPin.objectiveIndex),
                        tostring(data.mapPin.source),
                    }, ":")
                end
                for i = 1, math.min(#objectives, MAX_OBJECTIVES) do
                    local objective = objectives[i]
                    if type(objective) == "table" then
                        local entry = {
                            type = text(objective.type, 30),
                            objectiveType = number(objective.objectiveType),
                            fulfilled = number(objective.numFulfilled),
                            required = number(objective.numRequired),
                            finished = objective.finished == true,
                        }
                        if db.captureText then entry.text = text(objective.text, 300) end
                        data.objectives[#data.objectives + 1] = entry
                        fingerprint[#fingerprint + 1] = table.concat({
                            entry.type or "", entry.text or "", tostring(entry.fulfilled),
                            tostring(entry.required), tostring(entry.finished),
                        }, ":")
                    end
                end
                local key = questKey(id)
                local digest = table.concat(fingerprint, "|")
                if db.questFingerprints[key] ~= digest then
                    local before = #db.records
                    record("quest_objectives", data)
                    if #db.records > before then db.questFingerprints[key] = digest end
                end
            end
        end
    end
end

local function scheduleQuestScan()
    if questScanPending then return end
    if C_Timer and type(C_Timer.After) == "function" then
        questScanPending = true
        C_Timer.After(1, function()
            questScanPending = false
            local ok = pcall(scanQuestObjectives)
            if not ok and db then db.errors = (db.errors or 0) + 1 end
        end)
    else
        scanQuestObjectives()
    end
end

local function status()
    message(string.format("%s; %s; %d/%d saved; %d dropped; %d handler errors; full text %s.",
        VERSION, db.enabled and "on" or "off", #db.records, MAX_RECORDS,
        db.dropped or 0, db.errors or 0, db.captureText and "on" or "off"))
end

local function command(value)
    value = string.lower(text(value, 100) or "")
    if value == "on" then
        db.enabled = true
        db.collectionChoice = "on"
        message("collection on. Use /rrc off to pause it.")
        scheduleQuestScan()
    elseif value == "off" then
        db.enabled = false
        db.collectionChoice = "off"
        message("collection paused. Saved observations remain on this PC. Use /rrc on to resume.")
    elseif value == "text on" then
        db.captureText = true
        message("full visible NPC and quest text will be saved locally.")
    elseif value == "text off" then
        db.captureText = false
        message("full visible NPC and quest text collection is off.")
    elseif value == "clear" then
        db.records = {}
        db.questFingerprints = {}
        db.questRepFingerprints = {}
        db.questTextFingerprints = {}
        db.recipeFingerprints = {}
        db.professionFingerprints = {}
        db.sightingKeys = {}
        db.sightingCount = 0
        db.dropped = 0
        db.fullNotice = false
        message("saved observations cleared on this PC.")
    elseif value == "status" or value == "" then
        status()
    elseif value == "scan" then
        scheduleQuestScan()
        message("active quest objectives and available map pins queued for a local snapshot.")
    else
        message("commands: /rrc on, off, status, scan, text on, text off, clear")
    end
end

local handlers = {
    GOSSIP_SHOW = gossip,
    DYNAMIC_GOSSIP_POI_UPDATED = gossipPoi,
    GOSSIP_CONFIRM = function(gossipID, confirmationText, cost)
        local data = { npc = recentGossipNPC(), gossipID = number(gossipID),
            costCopper = number(cost) }
        if db.captureText then longText(data, "confirmationText", confirmationText) end
        record("gossip_confirmation", data)
    end,
    QUEST_GREETING = questGreeting,
    MERCHANT_SHOW = merchant,
    TRAINER_SHOW = trainer,
    TRAINER_UPDATE = trainer,
    TRADE_SKILL_SHOW = professionOpened,
    TRADE_SKILL_UPDATE = professionOpened,
    NEW_RECIPE_LEARNED = function(recipeID)
        recipeObservation(recipeID, "learned")
    end,
    TRADE_SKILL_DETAILS_UPDATE = function()
        local api = C_TradeSkillUI
        if api then recipeObservation(call(api.GetSelectedRecipeID), "viewed") end
    end,
    TAXIMAP_OPENED = taxi,
    QUEST_DETAIL = function(startItemID) quest("detail", startItemID) end,
    QUEST_PROGRESS = function() quest("progress") end,
    QUEST_COMPLETE = function() quest("complete") end,
    LOOT_OPENED = loot,
    LOOT_READY = loot,
    LOOT_SLOT_CHANGED = loot,
    LOOT_SLOT_CLEARED = function(slot)
        local observed = lootSlots[number(slot)]
        local item = observed and observed.item
        if item then
            local slotSequence = (db.nextSeq or 0) + 1
            record("loot_slot_cleared", {
                slot = number(slot), itemID = item.id, variant = item.variant,
                currencyID = item.currencyID, questID = item.questID,
                sourceKind = item.sourceKind, sourceID = item.sourceID,
                meaning = "loot_window_slot_cleared_not_confirmed_acquisition",
            })
            queueLootCheck(item, observed.countBefore, slotSequence)
            lootSlots[number(slot)] = nil
        end
    end,
    LOOT_CLOSED = function() lootSlots = {}; lastLootDigest = nil end,
    BAG_UPDATE_DELAYED = checkPendingLoot,
    PLAYER_ENTERING_WORLD = scheduleQuestScan,
    PLAYER_TARGET_CHANGED = targetSighting,
    QUEST_LOG_UPDATE = scheduleQuestScan,
    QUEST_ACCEPTED = function(questIndex, questID)
        local id = number(questID)
        local first = number(questIndex)
        if not id and first then
            id = questLogEntry(first)
            if not id and C_QuestLog and call(C_QuestLog.IsOnQuest, first) == true then
                id = first
            end
        end
        record("quest_state", { event = "accepted", id = id,
            questLogIndex = id and nil or first })
        scheduleQuestScan()
    end,
    QUEST_POI_UPDATE = scheduleQuestScan,
    PLAYER_INSIDE_QUEST_BLOB_STATE_CHANGED = function(questID, isInside)
        local id = number(questID)
        if not id or type(isInside) ~= "boolean" then return end
        local where = location()
        if not where.mapID or not where.x or not where.y then return end
        record("quest_area_edge_sample", {
            questID = id, playerInsideBlob = isInside,
            positionMeaning = "player_position_when_blob_membership_changed",
            geometry = "edge_sample_not_polygon",
        })
    end,
    QUEST_REMOVED = function(questID)
        record("quest_state", { event = "removed", id = number(questID) })
        if number(questID) then
            db.questFingerprints[questKey(questID)] = nil
            db.questRepFingerprints[questKey(questID)] = nil
            db.questTextFingerprints[questKey(questID)] = nil
        end
        scheduleQuestScan()
    end,
    QUEST_TURNED_IN = function(questID, xpReward, moneyReward)
        record("quest_state", { event = "turned_in", id = number(questID),
            xp = number(xpReward), moneyCopper = number(moneyReward) })
    end,
}

frame:SetScript("OnEvent", function(_, event, ...)
    if event == "ADDON_LOADED" then
        local name = ...
        if name ~= ADDON then return end
        if type(RestedRealmCollectorDB) ~= "table" then RestedRealmCollectorDB = {} end
        db = RestedRealmCollectorDB
        local _, build = call(GetBuildInfo)
        collectionScope = tostring(build or "unknown_build") .. ":"
            .. tostring(call(GetLocale) or "unknown_locale")
        db.schema = 1
        if type(db.records) ~= "table" then db.records = {} end
        if db.identitySchema ~= IDENTITY_SCHEMA then
            -- Older records stored the zone field as an NPC/object ID.
            -- Keep their other observations, but prevent a false entity join.
            local discardedIDs = 0
            for _, previous in ipairs(db.records) do
                if type(previous) == "table" and type(previous.data) == "table" then
                    local data = previous.data
                    if type(data.npc) == "table" then
                        if data.npc.id ~= nil then discardedIDs = discardedIDs + 1 end
                        data.npc.id = nil
                        data.npc.idStatus = "discarded_bad_guid_parse"
                    end
                    if type(data.items) == "table" then
                        for _, item in ipairs(data.items) do
                            if type(item) == "table" and item.sourceID ~= nil then
                                discardedIDs = discardedIDs + 1
                                item.sourceID = nil
                                item.sourceIDStatus = "discarded_bad_guid_parse"
                            end
                        end
                    end
                    previous.identitySchema = 1
                end
            end
            db.identitySchema = IDENTITY_SCHEMA
            if discardedIDs > 0 then
                message("corrected " .. discardedIDs .. " old NPC/loot IDs; other saved fields remain.")
            end
        end
        if db.gossipSelectionSchema ~= 2 or db.subtitleSchema ~= 2 then
            local lastGossip
            for _, previous in ipairs(db.records) do
                local data = type(previous.data) == "table" and previous.data or nil
                if data then
                    if db.subtitleSchema ~= 2 and type(data.npc) == "table"
                        and not data.npc.subtitle and type(data.npc.tooltipProbe) == "table" then
                        local probe = data.npc.tooltipProbe
                        data.npc.subtitle = subtitleBeforeLevel(probe.structured, data.npc.level)
                            or subtitleBeforeLevel(probe.private, data.npc.level)
                        if data.npc.subtitle then
                            data.npc.subtitleSource = "tooltip_role_before_level"
                            data.npc.tooltipProbe = nil
                        end
                    end
                    if previous.kind == "gossip" then lastGossip = previous end
                    if db.gossipSelectionSchema ~= 2
                        and previous.kind == "gossip_selection"
                        and data.selectionAPI == "SelectOptionByIndex"
                        and data.rawSelectionArgument == nil then
                        local raw = number(data.optionID)
                        data.rawSelectionArgument = raw
                        data.optionID = nil
                        data.selectionArgumentKind = "legacy_unresolved_order_index"
                        local previousData = lastGossip and lastGossip.data or nil
                        local sameNPC = previousData and previousData.npc and data.npc
                            and previousData.npc.id == data.npc.id
                        local closeInTime = lastGossip and previous.observedAt
                            and lastGossip.observedAt
                            and previous.observedAt >= lastGossip.observedAt
                            and previous.observedAt - lastGossip.observedAt <= 30
                        if sameNPC and closeInTime then
                            for _, option in ipairs(previousData.options or {}) do
                                if option.orderIndex == raw then
                                    data.optionID = option.id
                                    data.optionName = option.name
                                    data.optionOrderIndex = option.orderIndex
                                    data.selectionArgumentKind = "order_index_zero_based"
                                    break
                                end
                            end
                        end
                    end
                end
            end
            db.gossipSelectionSchema = 2
            db.subtitleSchema = 2
        end
        db.version = VERSION
        installGossipHooks()
        if type(db.nextSeq) ~= "number" then db.nextSeq = 0 end
        if type(db.questFingerprints) ~= "table" then db.questFingerprints = {} end
        if type(db.questRepFingerprints) ~= "table" then db.questRepFingerprints = {} end
        if type(db.questTextFingerprints) ~= "table" then db.questTextFingerprints = {} end
        if type(db.recipeFingerprints) ~= "table" then db.recipeFingerprints = {} end
        if type(db.professionFingerprints) ~= "table" then db.professionFingerprints = {} end
        if type(db.sightingKeys) ~= "table" then db.sightingKeys = {} end
        if db.sightingScope ~= collectionScope then
            db.sightingKeys = {}
            db.sightingCount = 0
            db.sightingScope = collectionScope
        end
        if type(db.sightingCount) ~= "number" then
            db.sightingCount = 0
            for _ in pairs(db.sightingKeys) do db.sightingCount = db.sightingCount + 1 end
        end
        -- On unless the player chose /rrc off. Consent to uploading is given in
        -- RestedRealm Companion's setup; the addon only notes things locally.
        -- Saves from before 0.1.14 stored "off" as a default rather than a
        -- choice, so they switch on once.
        db.enabled = db.collectionChoice ~= "off"
        if type(db.captureText) ~= "boolean" then db.captureText = true end
        SLASH_RESTEDREALMCOLLECTOR1 = "/rrc"
        SlashCmdList.RESTEDREALMCOLLECTOR = command
        if db.enabled then message("active; /rrc status shows saved observations, /rrc off pauses it.")
        else message("paused; type /rrc on to resume.") end
        return
    end
    if not db or not db.enabled then return end
    local handler = handlers[event]
    if handler then
        local ok = pcall(handler, ...)
        if not ok then db.errors = (db.errors or 0) + 1 end
    end
end)

frame:RegisterEvent("ADDON_LOADED")
for event in pairs(handlers) do
    local ok = pcall(frame.RegisterEvent, frame, event)
    if event == "PLAYER_INSIDE_QUEST_BLOB_STATE_CHANGED" then
        blobEventRegistered = ok
    end
end
