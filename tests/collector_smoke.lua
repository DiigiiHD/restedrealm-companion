-- Run with: lua tests/collector_smoke.lua addon/RestedRealmCollector/Collector.lua
local addonFile = assert(arg[1], "pass Collector.lua path")
local frame = {}
function frame:SetScript(_, callback) self.callback = callback end
function frame:RegisterEvent(_) end
CreateFrame = function() return frame end
SlashCmdList = {}
DEFAULT_CHAT_FRAME = { AddMessage = function() end }

local targetGuid = "Creature-0-0-0-99-2236-000000001"
local npcGuid
UnitGUID = function(unit)
    if unit == "npc" then return npcGuid end
    if unit == "target" then return targetGuid end
end
UnitName = function(unit)
    if unit == "npc" then return "Shelene Rhobart" end
    if unit == "target" then return "SensitiveName" end
end
C_TooltipInfo = { GetUnit = function()
    return { guid = npcGuid, lines = {
        { leftText = "Shelene Rhobart" },
        { leftText = "<Journeyman Leatherworker>" },
    } }
end }
GetMerchantNumItems = function() return 1 end
GetMerchantItemID = function() return 4604 end
C_MerchantFrame = { GetItemInfo = function()
    return { name = "Test item", price = 250, stackCount = 2,
        numAvailable = -1, isPurchasable = true, isUsable = false,
        hasExtendedCost = false }
end }
GetBuildInfo = function() return "1.60.1", "70009" end
GetLocale = function() return "enUS" end
GetServerTime = function() return 1790332912 end
GetNumLootItems = function() return 1 end
GetLootSlotInfo = function()
    return nil, "Test loot", 1, nil, 1, false, true, 375, true, false
end
GetLootSlotLink = function() return "item:2876" end
GetLootSourceInfo = function()
    return "Creature-0-0-0-99-1553-000000002", 1,
        "Creature-0-0-0-99-1554-000000003", 2
end
GetNumQuestLogEntries = function() return 1 end
GetQuestLogTitle = function()
    return "The Chill of Death", 8, nil, false, false, false, false, false, 375
end
local objectiveCount = 0
C_QuestLog = { GetQuestObjectives = function()
    return { { type = "item", text = "Duskbat Pelts: " .. objectiveCount .. "/5",
        numFulfilled = objectiveCount, numRequired = 5, finished = false } }
end, GetMapForQuestPOIs = function() return 1420 end,
GetQuestsOnMap = function() return { { questID = 375, x = 0.4, y = 0.6, numObjectives = 1 } } end,
GetNumQuestLogEntries = function() return 1 end,
GetInfo = function() return { questID = 375, title = "The Chill of Death", level = 8 } end,
GetQuestDifficultyLevel = function() return 8 end }
C_Minimap = { IsInsideQuestBlob = function() return false end }
QuestPOIGetIconInfo = nil
local pendingTimer
C_Timer = { After = function(_, callback) pendingTimer = callback end }
local function flushTimer()
    local callback = assert(pendingTimer)
    pendingTimer = nil
    callback()
end

RestedRealmCollectorDB = { records = { {
    seq = 1, data = { npc = { id = 99 }, items = { { sourceID = 99, id = 2876 } } },
} }, nextSeq = 1 }
assert(loadfile(addonFile))("RestedRealmCollector")
frame.callback(frame, "ADDON_LOADED", "RestedRealmCollector")
-- On by default; a save from before 0.1.14 has no recorded choice and switches on.
assert(RestedRealmCollectorDB.enabled == true)
assert(RestedRealmCollectorDB.records[1].data.npc.id == nil)
assert(RestedRealmCollectorDB.records[1].data.items[1].sourceID == nil)
-- "/rrc off" is remembered as the player's choice and stops recording.
SlashCmdList.RESTEDREALMCOLLECTOR("off")
assert(RestedRealmCollectorDB.enabled == false and RestedRealmCollectorDB.collectionChoice == "off")
local before = #RestedRealmCollectorDB.records
frame.callback(frame, "MERCHANT_SHOW")
assert(#RestedRealmCollectorDB.records == before)
SlashCmdList.RESTEDREALMCOLLECTOR("on")
assert(RestedRealmCollectorDB.collectionChoice == "on")
frame.callback(frame, "MERCHANT_SHOW")
local first = RestedRealmCollectorDB.records[2]
assert(first and first.kind == "merchant")
assert(first.data.npc.id == 2236 and first.data.npc.name == "SensitiveName")
local item = first.data.items[1]
assert(item.id == 4604 and item.name == "Test item")
assert(item.priceCopper == 250 and item.stackCount == 2)
assert(item.available == -1 and item.purchasable == true)
assert(item.usable == false and item.extendedCost == false)

targetGuid = "Player-0-0-0-0-000000001"
frame.callback(frame, "MERCHANT_SHOW")
local second = RestedRealmCollectorDB.records[3]
assert(second.data.npc.id == nil and second.data.npc.name == nil)
frame.callback(frame, "LOOT_OPENED")
local loot = RestedRealmCollectorDB.records[4]
assert(loot.kind == "loot_window")
assert(loot.data.items[1].sourceID == 1553)
assert(#loot.data.items[1].sources == 2 and loot.data.items[1].sources[2].id == 1554)
assert(loot.data.items[1].isQuestItem == true and loot.data.items[1].questID == 375)
assert(loot.identitySchema == 2)
frame.callback(frame, "LOOT_READY")
assert(#RestedRealmCollectorDB.records == 4)
frame.callback(frame, "LOOT_CLOSED")
frame.callback(frame, "QUEST_LOG_UPDATE")
flushTimer()
local objective = RestedRealmCollectorDB.records[5]
assert(objective.kind == "quest_objectives" and objective.data.id == 375)
assert(objective.data.objectives[1].type == "item")
assert(objective.data.mapPin.geometry == "pin_not_area")
assert(objective.data.mapPin.mapID == 1420)
assert(RestedRealmCollectorDB.questFingerprints["70009:enUS:375"] ~= nil)
frame.callback(frame, "QUEST_LOG_UPDATE")
flushTimer()
assert(#RestedRealmCollectorDB.records == 5)
objectiveCount = 1
frame.callback(frame, "QUEST_LOG_UPDATE")
flushTimer()
assert(#RestedRealmCollectorDB.records == 6)
assert(RestedRealmCollectorDB.records[6].data.objectives[1].fulfilled == 1)
frame.callback(frame, "QUEST_ACCEPTED", 1)
assert(RestedRealmCollectorDB.records[7].kind == "quest_state")
assert(RestedRealmCollectorDB.records[7].data.event == "accepted")
assert(RestedRealmCollectorDB.records[7].data.id == 375)
flushTimer()
GetQuestID = function() return 375 end
npcGuid = "Creature-0-0-0-99-3549-000000003"
GetQuestUiMapID = function() return 1420 end
GetQuestPOIBlobCount = function() return 1 end
C_QuestLog.IsOnMap = function() return true, true end
GetTitleText = function() return "The Chill of Death" end
GetNumQuestItems = function() return 1 end
GetNumQuestRewards = function() return 1 end
GetNumQuestChoices = function() return 2 end
GetQuestItemInfo = function(rewardType, i)
    if rewardType == "required" then return "Duskbat Pelt", nil, 5, 1, true, 2876 end
    if rewardType == "reward" then return "Test money item", nil, 1, 1, true, 9001 end
    return "Test choice " .. i, nil, 1, 2, true, 9100 + i
end
GetQuestItemLink = function() return "|Hitem:2876:0:0:0|h[Test]|h" end
GetRewardXP = function() return 650 end
GetRewardMoney = function() return 300 end
GetRewardHonor = function() return 20 end
GetRewardTitle = function() return 123 end
GetNumRewardCurrencies = function() return 1 end
GetQuestCurrencyInfo = function() return "Test currency", nil, 7, 78 end
GetNumQuestLogChoices = function() return 2 end
GetQuestLogChoiceInfo = function(i) return "Test choice " .. i, nil, 1, 2, true, 9100 + i end
C_QuestLog.GetQuestRewardCurrencies = function()
    return { { currencyID = 78, name = "Test currency", totalRewardAmount = 9,
        baseRewardAmount = 7, bonusRewardAmount = 2 } }
end
C_QuestInfoSystem = { GetQuestRewardSpells = function() return { 456 } end }
frame.callback(frame, "QUEST_DETAIL")
local detail = RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records]
assert(detail.kind == "quest" and detail.data.id == 375)
assert(detail.data.level == 8)
assert(detail.data.items[1].id == 2876 and detail.data.items[1].type == "required")
assert(detail.data.items[2].id == 9001 and detail.data.items[2].type == "reward")
assert(detail.data.items[3].id == 9101 and detail.data.items[3].type == "choice")
assert(detail.data.xp == 650 and detail.data.moneyCopper == 300)
assert(detail.data.npc.id == 3549 and detail.data.npc.subtitle == "Journeyman Leatherworker")
assert(detail.data.currencies[1].id == 78 and detail.data.currencies[1].quantity == 7)
assert(detail.data.spellRewardIDs[1] == 456)
assert(detail.data.honor == 20 and detail.data.titleRewardID == 123)
frame.callback(frame, "QUEST_LOG_UPDATE")
flushTimer()
local rewarded = RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records]
assert(rewarded.kind == "quest_objectives")
assert(#rewarded.data.rewards.choice == 2)
assert(rewarded.data.rewards.currency[1].quantity == 9)
assert(rewarded.data.rewards.spells[1] == 456)
assert(rewarded.data.mapHints.blobCount == 1)
assert(rewarded.data.mapHints.playerInsideBlob == false)
assert(rewarded.data.mapHints.geometry == "none")
GetQuestLogSelection = function() return 1 end
GetNumQuestLogRewardFactions = function() return 1 end
GetQuestLogRewardFactionInfo = function() return 68, 2500 end
frame.callback(frame, "QUEST_LOG_UPDATE")
flushTimer()
local reputation = RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records]
assert(reputation.kind == "quest_reputation" and reputation.data.id == 375)
assert(reputation.data.factions[1].id == 68 and reputation.data.factions[1].rawAmount == 2500)
local recordCount = #RestedRealmCollectorDB.records
frame.callback(frame, "QUEST_LOG_UPDATE")
flushTimer()
assert(#RestedRealmCollectorDB.records == recordCount)
GetNumAvailableQuests = function() return 1 end
GetAvailableTitle = function() return "The Chill of Death" end
GetAvailableQuestInfo = function() return false, 1, false, false, 375 end
GetAvailableLevel = function() return 8 end
GetNumActiveQuests = function() return 1 end
GetActiveTitle = function() return "Another quest", true end
GetActiveQuestID = function() return 376 end
GetActiveLevel = function() return 9 end
frame.callback(frame, "QUEST_GREETING")
local greeting = RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records]
assert(greeting.kind == "quest_greeting")
assert(greeting.data.available[1].id == 375 and greeting.data.available[1].level == 8)
assert(greeting.data.active[1].id == 376 and greeting.data.active[1].level == 9)
GetNumTrainerServices = function() return 1 end
GetTrainerServiceInfo = function() return "Test training", "available" end
GetTrainerServiceCost = function() return 500 end
frame.callback(frame, "TRAINER_SHOW")
local training = RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records]
assert(training.kind == "trainer" and training.data.services[1].priceCopper == 500)
GetNumTrainerServices = function() return 142 end
frame.callback(frame, "TRAINER_UPDATE")
local largeTrainer = RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records]
assert(largeTrainer.kind == "trainer" and #largeTrainer.data.services == 142)
assert(largeTrainer.data.truncated == false)
GetNumTrainerServices = function() return 0 end
frame.callback(frame, "TRAINER_SHOW")
local emptyTrainer = RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records]
assert(emptyTrainer.kind == "trainer_window" and emptyTrainer.data.shownCount == 0)
local emptyTrainerCount = #RestedRealmCollectorDB.records
frame.callback(frame, "TRAINER_UPDATE")
assert(#RestedRealmCollectorDB.records == emptyTrainerCount)
NumTaxiNodes = function() return 1 end
TaxiNodePosition = function() return 0.2, 0.3 end
TaxiNodeName = function() return "Test flight point" end
TaxiNodeGetType = function() return "CURRENT" end
frame.callback(frame, "TAXIMAP_OPENED")
local flight = RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records]
assert(flight.kind == "taxi_map" and flight.data.nodes[1].x == 0.2)
local bagCount = 0
GetItemCount = function() return bagCount end
frame.callback(frame, "LOOT_OPENED")
frame.callback(frame, "LOOT_SLOT_CLEARED", 1)
local cleared = RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records]
assert(cleared.kind == "loot_slot_cleared" and cleared.data.itemID == 2876)
assert(cleared.data.meaning == "loot_window_slot_cleared_not_confirmed_acquisition")
bagCount = 1
flushTimer()
local increase = RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records]
assert(increase.kind == "item_count_increase" and increase.data.quantityDelta == 1)
assert(increase.data.lootSlotSequence == cleared.seq)
frame.callback(frame, "LOOT_CLOSED")
frame.callback(frame, "LOOT_OPENED")
frame.callback(frame, "LOOT_SLOT_CLEARED", 1)
local secondClear = RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records]
bagCount = 2
frame.callback(frame, "BAG_UPDATE_DELAYED")
local secondIncrease = RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records]
assert(secondIncrease.kind == "item_count_increase")
assert(secondIncrease.data.lootSlotSequence == secondClear.seq)
assert(secondIncrease.data.quantityDelta == 1)
GetQuestLogQuestText = function() return "Long quest description", "Quest objectives prose" end
frame.callback(frame, "QUEST_LOG_UPDATE")
flushTimer()
local questText = RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records]
assert(questText.kind == "quest_log_text" and questText.data.id == 375)
assert(questText.data.description == "Long quest description")
C_GossipInfo = {
    GetText = function() return "Test dialogue" end,
    GetOptions = function() return { { gossipOptionID = 42, name = "Ask a question",
        status = 0, flags = 2, orderIndex = 2 } } end,
    GetAvailableQuests = function() return {} end,
    GetActiveQuests = function() return {} end,
    SelectOption = function() end,
    SelectOptionByIndex = function() end,
}
hooksecurefunc = function(target, method, callback)
    local original = target[method]
    target[method] = function(...)
        original(...)
        callback(...)
    end
end
frame.callback(frame, "GOSSIP_SHOW")
C_GossipInfo.SelectOption(42)
local selection = RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records]
assert(selection.kind == "gossip_selection" and selection.data.optionID == 42)
assert(selection.data.npc.id == 3549)
C_GossipInfo.SelectOptionByIndex(2)
local indexedSelection = RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records]
assert(indexedSelection.data.rawSelectionArgument == 2)
assert(indexedSelection.data.optionID == 42)
assert(indexedSelection.data.optionName == "Ask a question")
assert(indexedSelection.data.optionOrderIndex == 2)
C_TradeSkillUI = {
    GetBaseProfessionInfo = function() return { professionID = 165,
        professionName = "Leatherworking" } end,
    GetChildProfessionInfo = function() return { professionID = 165,
        professionName = "Leatherworking", skillLevel = 75, maxSkillLevel = 150 } end,
    GetRecipeInfo = function(id) return { name = "Test recipe", recipeID = id,
        learned = true, categoryID = 1 } end,
    GetTradeSkillLineForRecipe = function() return 165, "Leatherworking", 165 end,
    GetRecipeSchematic = function()
        return { outputItemID = 9200, quantityMin = 1, quantityMax = 1,
            reagentSlotSchematics = { { quantityRequired = 2,
                reagents = { { itemID = 4306 } } } } }
    end,
    GetSelectedRecipeID = function() return 9000 end,
}
frame.callback(frame, "TRADE_SKILL_SHOW")
local profession = RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records]
assert(profession.kind == "profession_opened" and profession.data.skillLineID == 165)
frame.callback(frame, "TRADE_SKILL_DETAILS_UPDATE")
local recipe = RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records]
assert(recipe.kind == "recipe" and recipe.data.id == 9000)
assert(recipe.data.outputItemID == 9200)
assert(recipe.data.reagents[1].alternatives[1].itemID == 4306)
local beforeDuplicate = #RestedRealmCollectorDB.records
frame.callback(frame, "TRADE_SKILL_DETAILS_UPDATE")
assert(#RestedRealmCollectorDB.records == beforeDuplicate)
frame.callback(frame, "NEW_RECIPE_LEARNED", 9001)
assert(RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records].data.event == "learned")
C_TradeSkillUI = nil
GetTradeSkillLine = function() return "Leatherworking", 75, 150 end
GetNumTradeSkills = function() return 2 end
GetTradeSkillInfo = function(i)
    if i == 1 then return "Leatherworking", "header" end
    return "Leather Belt", "optimal"
end
GetTradeSkillItemLink = function() return "|Hitem:4237:0|h[Belt]|h" end
GetTradeSkillRecipeLink = function() return "|Henchant:2372|h[Recipe]|h" end
GetTradeSkillNumMade = function() return 1, 1 end
GetTradeSkillNumReagents = function() return 1 end
GetTradeSkillReagentInfo = function() return "Leather", nil, 2, 99 end
GetTradeSkillReagentItemLink = function() return "|Hitem:2318:0|h[Leather]|h" end
frame.callback(frame, "TRADE_SKILL_SHOW")
local catalog = RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records]
assert(catalog.kind == "profession_catalog")
assert(catalog.data.recipes[1].outputItemID == 4237)
assert(catalog.data.recipes[1].recipeSpellID == 2372)
assert(catalog.data.recipes[1].reagents[1].id == 2318)
assert(catalog.data.recipes[1].reagents[1].required == 2)
local catalogCount = #RestedRealmCollectorDB.records
frame.callback(frame, "TRADE_SKILL_UPDATE")
assert(#RestedRealmCollectorDB.records == catalogCount)
C_Map = {
    GetBestMapForUnit = function() return 1420 end,
    GetPlayerMapPosition = function()
        return { GetXY = function() return 0.65, 0.61 end }
    end,
}
C_GossipInfo.GetPoiForUiMapID = function(mapID)
    if mapID == 1420 then return 77 end
end
C_GossipInfo.GetPoiInfo = function(mapID, poiID)
    if mapID == 1420 and poiID == 77 then
        return { name = "The inn", textureIndex = 3,
            position = { GetXY = function() return 0.4, 0.6 end } }
    end
end
frame.callback(frame, "DYNAMIC_GOSSIP_POI_UPDATED")
local poi = RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records]
assert(poi.kind == "gossip_poi" and poi.data.poiID == 77)
assert(poi.data.x == 0.4 and poi.data.y == 0.6)
targetGuid = "Creature-0-0-0-99-1553-000000004"
frame.callback(frame, "PLAYER_TARGET_CHANGED")
local sighting = RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records]
assert(sighting.kind == "entity_sighting" and sighting.data.entity.id == 1553)
assert(sighting.data.positionMeaning == "player_location_while_targeting_entity")
frame.callback(frame, "PLAYER_INSIDE_QUEST_BLOB_STATE_CHANGED", 375, true)
local edge = RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records]
assert(edge.kind == "quest_area_edge_sample" and edge.data.questID == 375)
assert(edge.data.geometry == "edge_sample_not_polygon")
local sightingCount = #RestedRealmCollectorDB.records
frame.callback(frame, "PLAYER_TARGET_CHANGED")
assert(#RestedRealmCollectorDB.records == sightingCount)
C_TooltipInfo.GetUnit = function() return { guid = npcGuid,
    lines = { { leftText = "Shelene Rhobart" }, { leftText = "Level 25" } } } end
_G.RestedRealmCollectorScannerTooltipTextLeft2 = { GetText = function()
    return "<Journeyman Leatherworker>" end }
CreateFrame = function(kind)
    if kind == "GameTooltip" then
        return { SetOwner = function() end, ClearLines = function() end,
            SetUnit = function() end, NumLines = function() return 2 end }
    end
    return frame
end
frame.callback(frame, "QUEST_DETAIL")
local fallback = RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records]
assert(fallback.data.npc.subtitle == "Journeyman Leatherworker")
assert(fallback.data.npc.subtitleSource == "private_unit_tooltip")
_G.RestedRealmCollectorScannerTooltipTextLeft2.GetText = function()
    return "Profession trainer" end
frame.callback(frame, "QUEST_DETAIL")
local diagnostic = RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records]
assert(diagnostic.data.npc.subtitle == nil)
assert(diagnostic.data.npc.tooltipProbe.structured[1] == "Level 25")
assert(diagnostic.data.npc.tooltipProbe.private[1] == "Profession trainer")
UnitLevel = function(unit) if unit == "npc" then return 16 end end
C_TooltipInfo.GetUnit = function() return { guid = npcGuid, lines = {
    { leftText = "NPC name" }, { leftText = "Paladin Trainer" },
    { leftText = "Level 16" }, { leftText = "Undercity" } } } end
frame.callback(frame, "QUEST_DETAIL")
local plainRole = RestedRealmCollectorDB.records[#RestedRealmCollectorDB.records]
assert(plainRole.data.npc.subtitle == "Paladin Trainer")
assert(plainRole.data.npc.subtitleSource == "tooltip_role_before_level")
SlashCmdList.RESTEDREALMCOLLECTOR("clear")
assert(#RestedRealmCollectorDB.records == 0)
assert(RestedRealmCollectorDB.sightingCount == 0)
assert(next(RestedRealmCollectorDB.questFingerprints) == nil)
print("collector smoke passed")
