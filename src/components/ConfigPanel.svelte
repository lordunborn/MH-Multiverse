<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { openPath } from '@tauri-apps/plugin-opener'
  import { invoke } from '@tauri-apps/api/core'
  import { appConfig, setConsolePresets, serverRunning, setShutdownConfig, logFilterThreshold, type ShutdownConfig } from '../lib/store'
  import { FALLBACK_COMMANDS } from '../lib/serverCommands'
  import PanelSidebar from './PanelSidebar.svelte'

  export let embedded = false
  export let onBack: (() => void) | null = null

  // -- Types --

  type FieldType = 'bool' | 'number' | 'string' | 'textarea'

  interface Field {
    key: string
    section: string
    type: FieldType
    label: string
    description: string
    min?: number
    max?: number
  }

  interface SubSection {
    title: string
    fields: Field[]
  }

  interface NavSection {
    id: string
    label: string
    fields?: Field[]
    subsections?: SubSection[]
  }

  type IniData = Record<string, Record<string, string>>

  // -- Helper --

  let openDirError = ''

  async function openConfigDir() {
    if (!$appConfig.server_exe) return
    openDirError = ''
    try {
      const dir = await invoke<string>('get_config_dir', { serverExe: $appConfig.server_exe })
      await openPath(dir)
    } catch (e) {
      openDirError = String(e)
    }
  }

  // -- Config schema --

  // Nav id/label/title all use the LITERAL Config.ini section name (e.g. "MTXStore"),
  // not a friendlier paraphrase -- the whole point is that a setting's location in
  // this app matches what someone would find searching Config.ini directly.
  const schema: NavSection[] = [
    {
      id: 'Logging',
      label: 'Logging',
      fields: [
        { key: 'EnableLogging',            section: 'Logging', type: 'bool',   label: 'Enable Logging',              description: 'Enables logging.' },
        { key: 'SynchronousMode',          section: 'Logging', type: 'bool',   label: 'Synchronous Mode',            description: 'Enables synchronous output for logging (slow, for debugging only).' },
        { key: 'HideSensitiveInformation', section: 'Logging', type: 'bool',   label: 'Hide Sensitive Information',  description: 'Hides sensitive information (emails / IP addresses) from logs.' },
        { key: 'EnableConsole',            section: 'Logging', type: 'bool',   label: 'Enable Console',              description: 'Enables the console output target.' },
        { key: 'ConsoleIncludeTimestamps', section: 'Logging', type: 'bool',   label: 'Console Include Timestamps',  description: 'Enables message timestamps for the console output target.' },
        { key: 'ConsoleMinLevel',          section: 'Logging', type: 'number', label: 'Console Min Level',           description: 'Sets the minimum logging level for the console output target [0-5].', min: 0, max: 5 },
        { key: 'ConsoleMaxLevel',          section: 'Logging', type: 'number', label: 'Console Max Level',           description: 'Sets the maximum logging level for the console output target [0-5].', min: 0, max: 5 },
        { key: 'ConsoleChannels',          section: 'Logging', type: 'string', label: 'Console Channels',            description: 'Sets log message sources that the console target accepts.' },
        { key: 'EnableFile',               section: 'Logging', type: 'bool',   label: 'Enable File',                 description: 'Enables the file output target.' },
        { key: 'FileIncludeTimestamps',    section: 'Logging', type: 'bool',   label: 'File Include Timestamps',     description: 'Enables message timestamps for the file output target.' },
        { key: 'FileMinLevel',             section: 'Logging', type: 'number', label: 'File Min Level',              description: 'Sets the minimum logging level for the file output target [0-5].', min: 0, max: 5 },
        { key: 'FileMaxLevel',             section: 'Logging', type: 'number', label: 'File Max Level',              description: 'Sets the maximum logging level for the file output target [0-5].', min: 0, max: 5 },
        { key: 'FileChannels',             section: 'Logging', type: 'string', label: 'File Channels',               description: 'Sets log message sources that the file target accepts.' },
        { key: 'FileSplitOutput',          section: 'Logging', type: 'bool',   label: 'File Split Output',           description: 'Splits output into separate files based on message category.' },
      ],
    },
    {
      id: 'Frontend',
      label: 'Frontend',
      fields: [
        { key: 'BindIP',           section: 'Frontend', type: 'string', label: 'Bind IP',            description: 'IP address for the frontend server to bind to. Set to 0.0.0.0 to listen on all network interfaces.' },
        { key: 'Port',             section: 'Frontend', type: 'number', label: 'Port',                description: 'Port for the frontend server to bind to.', min: 1, max: 65535 },
        { key: 'PublicAddress',   section: 'Frontend', type: 'string', label: 'Public Address',       description: 'An address of the frontend server that needs to be reachable for clients. Can be an IP address (e.g. 192.168.1.2) or a URL (e.g. fes2.marvelheroes.com).' },
        { key: 'ReceiveTimeoutMS', section: 'Frontend', type: 'number', label: 'Receive Timeout (ms)', description: "Number of milliseconds the server waits before closing the connection when trying to receive data from a client. Set to 0 to wait indefinitely.", min: 0 },
        { key: 'SendTimeoutMS',    section: 'Frontend', type: 'number', label: 'Send Timeout (ms)',    description: "Number of milliseconds the server waits before closing the connection when trying to send data to a client. Set to 0 to wait indefinitely.", min: 0 },
      ],
    },
    {
      id: 'WebFrontend',
      label: 'WebFrontend',
      fields: [
        { key: 'Address',               section: 'WebFrontend', type: 'string', label: 'Address',                   description: 'Address for the web frontend to listen on.' },
        { key: 'Port',                   section: 'WebFrontend', type: 'number', label: 'Port',                       description: 'Port for the web frontend to listen on.', min: 1, max: 65535 },
        { key: 'EnableLoginRateLimit',   section: 'WebFrontend', type: 'bool',   label: 'Enable Login Rate Limit',    description: 'Enables rate limiting for login attempts.' },
        { key: 'LoginRateLimitCostMS',   section: 'WebFrontend', type: 'number', label: 'Login Rate Limit Cost (ms)', description: 'Time cost in milliseconds for a single login attempt.', min: 0 },
        { key: 'LoginRateLimitBurst',    section: 'WebFrontend', type: 'number', label: 'Login Rate Limit Burst',     description: 'The number of times in a row the time cost can be paid before the limit starts applying. Must be >= 2.', min: 2 },
        { key: 'EnableWebApi',           section: 'WebFrontend', type: 'bool',   label: 'Enable Web API',             description: 'Enables custom web API endpoints.' },
        { key: 'EnableDashboard',       section: 'WebFrontend', type: 'bool',   label: 'Enable Dashboard',            description: 'Enables a server dashboard accessible from the web API. Requires the web API to be enabled.' },
        { key: 'DashboardFileDirectory', section: 'WebFrontend', type: 'string', label: 'Dashboard File Directory',   description: 'Name of the directory that contains dashboard files. The directory should be placed in Data\\Web.' },
        { key: 'DashboardUrlPath',       section: 'WebFrontend', type: 'string', label: 'Dashboard URL Path',         description: 'URL path relative to the origin from which the dashboard is served.' },
      ],
    },
    {
      id: 'PlayerManager',
      label: 'PlayerManager',
      fields: [
        { key: 'EnablePersistence',           section: 'PlayerManager', type: 'bool',   label: 'Enable Persistence',            description: 'Allows player data to be saved.' },
        { key: 'UseJsonDBManager',             section: 'PlayerManager', type: 'bool',   label: 'Use JSON DB Manager',            description: 'Switches to JSON-based backend for saving player data. NOTE: JSON backend supports only a single account / user.' },
        { key: 'AllowClientVersionMismatch',   section: 'PlayerManager', type: 'bool',   label: 'Allow Client Version Mismatch',  description: 'Allows clients that do not match the server\'s version of the game to connect.' },
        { key: 'UseWhitelist',                 section: 'PlayerManager', type: 'bool',   label: 'Use Whitelist',                  description: 'When enabled, only clients added to the whitelist via the !account whitelist command are allowed to log in.' },
        { key: 'ShowNewsOnLogin',              section: 'PlayerManager', type: 'bool',   label: 'Show News On Login',             description: "Requests the client to open the news windows after logging in." },
        { key: 'NewsUrl',                      section: 'PlayerManager', type: 'string', label: 'News URL',                       description: "News URL for the client's news window." },
        { key: 'ServerCapacity',               section: 'PlayerManager', type: 'number', label: 'Server Capacity',                description: 'Indicates the maximum concurrent number of players allowed on the server. Players who exceed this capacity will be put in a queue. Values <= 0 indicate unlimited capacity.' },
        { key: 'MaxLoginQueueClients',        section: 'PlayerManager', type: 'number', label: 'Max Login Queue Clients',        description: 'The maximum number of normal priority clients that can be put into the login queue at the same time.', min: 0 },
        { key: 'EnableTownPlayerLimit',       section: 'PlayerManager', type: 'bool',   label: 'Enable Town Player Limit',       description: 'When enabled, town regions respect player limits defined in region prototypes.' },
      ],
    },
    {
      id: 'SQLiteDBManager',
      label: 'SQLiteDBManager',
      fields: [
        { key: 'FileName',              section: 'SQLiteDBManager', type: 'string', label: 'File Name',              description: "Name of the SQLite database file to use relative to the server's Data directory." },
        { key: 'MaxBackupNumber',       section: 'SQLiteDBManager', type: 'number', label: 'Max Backup Number',      description: 'The maximum number of backup files to create (oldest backups will be overwritten). Set this to 0 to disable backups.', min: 0 },
        { key: 'BackupIntervalMinutes', section: 'SQLiteDBManager', type: 'number', label: 'Backup Interval (min)',  description: 'The minimum interval in minutes between backup creation.', min: 1 },
      ],
    },
    {
      id: 'JsonDBManager',
      label: 'JsonDBManager',
      fields: [
        { key: 'FileName',              section: 'JsonDBManager', type: 'string', label: 'File Name',             description: "Name of the JSON save file to use relative to the server's Data directory." },
        { key: 'MaxBackupNumber',       section: 'JsonDBManager', type: 'number', label: 'Max Backup Number',     description: 'The maximum number of backup files to create (oldest backups will be overwritten). Set this to 0 to disable backups.', min: 0 },
        { key: 'BackupIntervalMinutes', section: 'JsonDBManager', type: 'number', label: 'Backup Interval (min)', description: 'The minimum interval in minutes between backup creation.', min: 1 },
        { key: 'PlayerName',            section: 'JsonDBManager', type: 'string', label: 'Player Name',           description: 'Player name to assign to the account loaded from JSON.' },
      ],
    },
    {
      id: 'GroupingManager',
      label: 'GroupingManager',
      fields: [
        { key: 'ServerName',            section: 'GroupingManager', type: 'string',   label: 'Server Name',             description: 'Name in chat for system messages sent by the server.' },
        { key: 'ServerPrestigeLevel',   section: 'GroupingManager', type: 'number',   label: 'Server Prestige Level',   description: 'Determines the color of the server\'s name in system messages. 0 - white, 1 - green, 2 - blue, 3 - purple, 4 - orange, 5 - red, 6 - yellow (cosmic).', min: 0, max: 6 },
        { key: 'MotdText',              section: 'GroupingManager', type: 'textarea', label: 'MOTD Text',               description: 'Message of the day text.' },
        { key: 'LogPrivateChatRooms',   section: 'GroupingManager', type: 'bool',     label: 'Log Private Chat Rooms', description: 'When enabled, the text of messages in private chat rooms (tell, party, guild) is included in server logs.' },
        { key: 'EnableChatTips',        section: 'GroupingManager', type: 'bool',     label: 'Enable Chat Tips',        description: 'When enabled, the server periodically broadcasts chat tips to all players.' },
        { key: 'ChatTipFileName',       section: 'GroupingManager', type: 'string',   label: 'Chat Tip File Name',      description: 'Name of the file in the data directory that contains chat tips.' },
        { key: 'ChatTipIntervalMinutes',section: 'GroupingManager', type: 'number',   label: 'Chat Tip Interval (min)', description: 'Interval for broadcasting chat tips in minutes.', min: 1 },
        { key: 'ChatTipShuffle',        section: 'GroupingManager', type: 'bool',     label: 'Chat Tip Shuffle',        description: 'Randomizes the order of chat tips on load.' },
      ],
    },
    {
      id: 'GameInstance',
      label: 'GameInstance',
      fields: [
        { key: 'NumWorkerThreads', section: 'GameInstance', type: 'number', label: 'Num Worker Threads', description: 'The number of worker threads used to process game instances. This number ideally should not exceed the number of logical CPUs the machine running the server has.', min: 1 },
      ],
    },
    {
      id: 'GameData',
      label: 'GameData',
      fields: [
        { key: 'LoadAllPrototypes',          section: 'GameData', type: 'bool', label: 'Load All Prototypes',           description: 'Preloads all prototypes on server start. Makes server start slower, but removes lag spikes during the game when data is loaded for the first time.' },
        { key: 'UseEquipmentSlotTableCache', section: 'GameData', type: 'bool', label: 'Use Equipment Slot Table Cache', description: 'Enables caching for the EquipmentSlotTable. Makes server start slower if not used with Load All Prototypes.' },
        { key: 'EnablePatchManager',         section: 'GameData', type: 'bool', label: 'Enable Patch Manager',          description: 'Enables Prototype Patch Manager. Fixes mission bugs and prototype errors.' },
        { key: 'EnableLiveTuningEvents',     section: 'GameData', type: 'bool', label: 'Enable Live Tuning Events',     description: 'Applies events based on the current date when loading Live Tuning data.' },
        { key: 'AutoRefreshLiveTuning',      section: 'GameData', type: 'bool', label: 'Auto Refresh Live Tuning',      description: 'Automatically reloads Live Tuning data when the daily mission reset happens. Requires Enable Live Tuning Events.' },
        { key: 'LoadLocaleFiles',            section: 'GameData', type: 'bool', label: 'Load Locale Files',             description: 'Enables loading of locale files from Data\\Game\\Loco.' },
      ],
    },
    {
      id: 'GameOptions',
      label: 'GameOptions',
      fields: [
        { key: 'TeamUpSystemEnabled',                       section: 'GameOptions', type: 'bool',   label: 'Team-Up System Enabled',           description: 'Enables team-ups. Sent to the client and affects the UI.' },
        { key: 'AchievementsEnabled',                        section: 'GameOptions', type: 'bool',   label: 'Achievements Enabled',              description: 'Enables achievements. Sent to the client and affects the UI.' },
        { key: 'OmegaMissionsEnabled',                       section: 'GameOptions', type: 'bool',   label: 'Omega Missions Enabled',            description: 'Enables shared quests. Sent to the client and affects the UI.' },
        { key: 'VeteranRewardsEnabled',                      section: 'GameOptions', type: 'bool',   label: 'Veteran Rewards Enabled',           description: 'Enables login rewards. Sent to the client and affects the UI.' },
        { key: 'MultiSpecRewardsEnabled',                    section: 'GameOptions', type: 'bool',   label: 'Multi-Spec Rewards Enabled',        description: 'Enables multi-specs. Sent to the client and affects the UI.' },
        { key: 'GiftingEnabled',                             section: 'GameOptions', type: 'bool',   label: 'Gifting Enabled',                   description: 'Enables gifting in the in-game store. Sent to the client and affects the UI.' },
        { key: 'CharacterSelectV2Enabled',                   section: 'GameOptions', type: 'bool',   label: 'Character Select V2 Enabled',       description: 'Enables the newer starting hero selection screen for new players. Falls back to V1 when disabled.' },
        { key: 'CommunityNewsV2Enabled',                     section: 'GameOptions', type: 'bool',   label: 'Community News V2 Enabled',         description: 'Enables the newer community news window. Falls back to V1 when disabled.' },
        { key: 'LeaderboardsEnabled',                        section: 'GameOptions', type: 'bool',   label: 'Leaderboards Enabled',              description: 'Enables leaderboards. Sent to the client and affects the UI.' },
        { key: 'NewPlayerExperienceEnabled',                 section: 'GameOptions', type: 'bool',   label: 'New Player Experience Enabled',     description: 'Enables the tutorial for new players.' },
        { key: 'MissionTrackerV2Enabled',                    section: 'GameOptions', type: 'bool',   label: 'Mission Tracker V2 Enabled',        description: 'Enables the newer mission tracker. Falls back to V1 when disabled.' },
        { key: 'GiftingAccountAgeInDaysRequired',            section: 'GameOptions', type: 'number', label: 'Gifting Account Age Required (days)', description: 'Number of days an account needs to exist for to unlock store gifting.', min: 0 },
        { key: 'GiftingAvatarLevelRequired',                 section: 'GameOptions', type: 'number', label: 'Gifting Avatar Level Required',     description: 'Minimum avatar level that needs to be reached to unlock store gifting.', min: 0 },
        { key: 'GiftingLoginCountRequired',                  section: 'GameOptions', type: 'number', label: 'Gifting Login Count Required',      description: 'Number of days an account needs to log in for to unlock store gifting.', min: 0 },
        { key: 'InfinitySystemEnabled',                      section: 'GameOptions', type: 'bool',   label: 'Infinity System Enabled',           description: 'Enables the Infinity system. Falls back to the Omega system when disabled.' },
        { key: 'ChatBanVoteAccountAgeInDaysRequired',        section: 'GameOptions', type: 'number', label: 'Chat Ban Vote Account Age Required (days)', description: 'Number of days an account needs to exist for to unlock chat spam reporting.', min: 0 },
        { key: 'ChatBanVoteAvatarLevelRequired',              section: 'GameOptions', type: 'number', label: 'Chat Ban Vote Avatar Level Required', description: 'Minimum avatar level that needs to be reached to unlock chat spam reporting.', min: 0 },
        { key: 'ChatBanVoteLoginCountRequired',               section: 'GameOptions', type: 'number', label: 'Chat Ban Vote Login Count Required', description: 'Number of days an account needs to log in for to unlock chat spam reporting.', min: 0 },
        { key: 'IsDifficultySliderEnabled',                   section: 'GameOptions', type: 'bool',   label: 'Difficulty Slider Enabled',          description: 'Enables region difficulty selection.' },
        { key: 'OrbisTrophiesEnabled',                        section: 'GameOptions', type: 'bool',   label: 'Orbis Trophies Enabled',             description: 'Enables PS4 trophy unlocks (unused in the PC version).' },
      ],
    },
    {
      id: 'CustomGameOptions',
      label: 'CustomGameOptions',
      subsections: [
        {
          title: 'General',
          fields: [
            { key: 'AutosaveIntervalMinutes',            section: 'CustomGameOptions', type: 'number', label: 'Autosave Interval (min)',           description: 'Determines the interval for saving player data when not transferring between regions. Set to <= 0 to disable autosaving.' },
            { key: 'ESCooldownOverrideMinutes',          section: 'CustomGameOptions', type: 'number', label: 'ES Cooldown Override (min)',        description: 'Overrides the default Eternity Splinter drop cooldown duration when the value is >= 0.' },
            { key: 'CombineESStacks',                    section: 'CustomGameOptions', type: 'bool',   label: 'Combine ES Stacks',                 description: 'Combine multiple Eternity Splinter stacks into a single item when they drop at the same time.' },
            { key: 'AutoUnlockAvatars',                  section: 'CustomGameOptions', type: 'bool',   label: 'Auto Unlock Avatars',                description: 'Automatically unlocks all avatars for players who have finished the tutorial.' },
            { key: 'AutoUnlockTeamUps',                  section: 'CustomGameOptions', type: 'bool',   label: 'Auto Unlock Team-Ups',               description: 'Automatically unlocks all team-ups for players who have finished the tutorial.' },
            { key: 'DisableMovementPowerChargeCost',     section: 'CustomGameOptions', type: 'bool',   label: 'Disable Movement Power Charge Cost', description: 'Disables charge costs for movement powers to imitate pre-BUE behavior.' },
            { key: 'AllowSameGroupTalents',               section: 'CustomGameOptions', type: 'bool',   label: 'Allow Same Group Talents',           description: 'Allows mutually exclusive talents to be enabled at the same time.' },
            { key: 'EnableCreditChestConversion',        section: 'CustomGameOptions', type: 'bool',   label: 'Enable Credit Chest Conversion',    description: 'Allows conversion of credits to sellable chest items via the !item creditchest command.' },
            { key: 'CreditChestConversionMultiplier',    section: 'CustomGameOptions', type: 'number', label: 'Credit Chest Conversion Multiplier',description: 'Cost multiplier for converting credits to chest items.', min: 0 },
            { key: 'DisableInstancedLoot',                section: 'CustomGameOptions', type: 'bool',   label: 'Disable Instanced Loot',             description: 'Makes loot free-for-all, Diablo 2 style.' },
            { key: 'LootSpawnGridCellRadius',             section: 'CustomGameOptions', type: 'number', label: 'Loot Spawn Grid Cell Radius',       description: 'Affects the spacing of loot that drops [16-128].', min: 16, max: 128 },
            { key: 'TrashedItemExpirationTimeMultiplier', section: 'CustomGameOptions', type: 'number', label: 'Trashed Item Expiration Multiplier',description: 'Affects how long items trashed by players remain on the ground.', min: 0 },
            { key: 'DisableAccountBinding',                section: 'CustomGameOptions', type: 'bool',   label: 'Disable Account Binding',            description: 'Disables account binding on pickup for items.' },
            { key: 'DisableCharacterBinding',             section: 'CustomGameOptions', type: 'bool',   label: 'Disable Character Binding',          description: 'Disables character binding on equip for items.' },
            { key: 'DisableMissionXPBonuses',             section: 'CustomGameOptions', type: 'bool',   label: 'Disable Mission XP Bonuses',        description: 'Prevents mission experience rewards from being affected by experience bonuses.' },
            { key: 'UsePrestigeLootTable',                section: 'CustomGameOptions', type: 'bool',   label: 'Use Prestige Loot Table',            description: 'Replaces starting costume prestige rewards with the loot table specified in game data.' },
            { key: 'EnableUltimatePrestige',              section: 'CustomGameOptions', type: 'bool',   label: 'Enable Ultimate Prestige',           description: 'Allows prestige level to be reset after reaching the prestige level cap.' },
            { key: 'ApplyHiddenPvPDamageModifiers',       section: 'CustomGameOptions', type: 'bool',   label: 'Apply Hidden PvP Damage Modifiers', description: 'Enables hidden damage modifiers based on K/D ratio, match history, and Omega level in PvP combat.' },
          ],
        },
        {
          title: 'Loot Filters',
          fields: [
            { key: 'EnableLootFilters', section: 'CustomGameOptions', type: 'bool', label: 'Enable Loot Filters', description: "Enables the '!filter' chat command system for Ring, Medal, and Insignia rarity-based filtering. On by default." },
          ],
        },
        {
          title: 'Item Auto Pickup Settings',
          fields: [
            { key: 'EnableItemAutoPickup',                        section: 'CustomGameOptions', type: 'bool',   label: 'Enable Item Auto Pickup',                    description: 'Auto-pickup currency-style items (Eternity Splinters, Cube Shards, Runestones, all Marks, etc.) within Item Auto Pickup Radius. Off by default.' },
            { key: 'ItemAutoPickupRadius',                        section: 'CustomGameOptions', type: 'number', label: 'Item Auto Pickup Radius',                    description: 'Radius (in world units) around the avatar within which currency items are auto-picked-up. Default 400 matches the InteractRange.', min: 0 },
            { key: 'ItemAutoPickupIntervalMs',                    section: 'CustomGameOptions', type: 'number', label: 'Item Auto Pickup Interval (ms)',             description: 'Server-side scan interval in milliseconds. Lower = more responsive, slightly more CPU. Floor 50 ms.', min: 50 },
            { key: 'EnableCraftingIngredientAutoPickup',          section: 'CustomGameOptions', type: 'bool',   label: 'Enable Crafting Ingredient Auto Pickup',     description: 'Auto-pickup nearby crafting ingredients (e.g. catalysts, crafting mats). Reuses Item Auto Pickup Radius.' },
            { key: 'CraftingIngredientAutoPickupToStash',        section: 'CustomGameOptions', type: 'bool',   label: 'Crafting Ingredient Auto Pickup To Stash',   description: 'When enabled, crafting ingredients picked up automatically are sent to the stash instead of general inventory.' },
            { key: 'EnableCraftingIngredientAutoPickupVerboseLogging', section: 'CustomGameOptions', type: 'bool', label: 'Crafting Ingredient Auto Pickup Verbose Logging', description: 'Logs verbose diagnostics for crafting ingredient auto-pickup decisions. Off by default.' },
            { key: 'EnableGlyphAutoPickup',                       section: 'CustomGameOptions', type: 'bool',   label: 'Enable Glyph Auto Pickup',                    description: 'Auto-pickup nearby Runeword Glyphs (Entity/Items/Runewords/Glyphs/). Reuses Item Auto Pickup Radius.' },
            { key: 'GlyphAutoPickupToStash',                      section: 'CustomGameOptions', type: 'bool',   label: 'Glyph Auto Pickup To Stash',                  description: 'When enabled, Glyphs picked up automatically are sent to the stash instead of general inventory.' },
            { key: 'EnableRelicAutoPickup',                       section: 'CustomGameOptions', type: 'bool',   label: 'Enable Relic Auto Pickup',                    description: 'Auto-pickup nearby Relics (Entity/Items/Relics/Prototypes/). Reuses Item Auto Pickup Radius.' },
            { key: 'RelicAutoPickupToStash',                      section: 'CustomGameOptions', type: 'bool',   label: 'Relic Auto Pickup To Stash',                  description: 'When enabled, Relics picked up automatically are sent to the stash instead of general inventory.' },
            { key: 'RelicAutoPickupEquipIfSameTypeEquipped',      section: 'CustomGameOptions', type: 'bool',   label: 'Relic Auto Pickup Equip If Same Type Equipped', description: 'When enabled, a picked-up Relic stacks directly onto an already-equipped Relic of the same type instead of going to the stash/inventory.' },
          ],
        },
        {
          title: 'Stash Affinity Settings',
          fields: [
            { key: 'StashAffinityEnable',        section: 'CustomGameOptions', type: 'bool', label: 'Stash Affinity Enable',         description: 'When enabled, manually moving an item into a stash tab is redirected to a better-matching stash tab (character-specific stash for bound/restricted items, or a tab whose custom display name matches the item\'s type, e.g. "Relic"/"Rune"/"Unique") if one exists and has space.' },
            { key: 'StashAffinityLoggingEnable', section: 'CustomGameOptions', type: 'bool', label: 'Stash Affinity Logging Enable', description: 'Logs every Stash Affinity redirect decision to Logs/StashAffinity/ for debugging. Off by default.' },
          ],
        },
        {
          title: 'Phantom Heroes Settings',
          fields: [
            { key: 'PhantomHeroesEnable',              section: 'CustomGameOptions', type: 'bool',   label: 'Phantom Heroes Enable',                 description: "Enables the '!phantom' command group (spawn/squad/costume/clear), which lets players summon AI-controlled hero companions that fight alongside them." },
            { key: 'PhantomHeroesLoggingEnable',        section: 'CustomGameOptions', type: 'bool',   label: 'Phantom Heroes Logging Enable',        description: 'Logs verbose Phantom Heroes diagnostics (spawn, revive, AI targeting, AOI sync, gear rolls, etc.) to the main log. Off by default - this is a firehose, only turn it on when actively debugging a phantom issue.' },
            { key: 'PhantomHeroesDespawnOnDeath',       section: 'CustomGameOptions', type: 'bool',   label: 'Phantom Heroes Despawn On Death',      description: "When enabled, phantoms take real damage and are despawned if they'd otherwise go down (they have no client to drive a revive flow). When disabled, phantoms are invulnerable instead." },
            { key: 'PhantomHeroesMaxActive',             section: 'CustomGameOptions', type: 'number', label: 'Phantom Heroes Max Active',            description: 'Maximum number of phantoms a single player can have active at once, across any number of spawn calls. Default 9 - a party/raid group is 10 total including the human caller.', min: 0 },
            { key: 'PhantomGearItemBlacklist',           section: 'CustomGameOptions', type: 'string', label: 'Phantom Gear Item Blacklist',          description: 'Comma-separated list of item prototype name substrings to exclude from phantom gear rolls (e.g. artifacts that are no longer obtainable in-game, purely cosmetic, or have broken/placeholder icons). Case-insensitive. Empty by default. Does not require a rebuild to change, only a restart.' },
            { key: 'PhantomHeroesDownedGracePeriodMs',  section: 'CustomGameOptions', type: 'number', label: 'Phantom Heroes Downed Grace Period (ms)', description: "How long (ms) a downed phantom stays revivable (like a real player's downed state) before it despawns if nobody revives it in time. Only applies when Phantom Heroes Despawn On Death is true. Default 30000 (30 seconds).", min: 0 },
            { key: 'PhantomHeroesReviveCooldownMs',      section: 'CustomGameOptions', type: 'number', label: 'Phantom Heroes Revive Cooldown (ms)',   description: 'How long (ms) after a phantom revives the HUMAN player before another phantom-driven revive of the human can happen again. Does not apply to phantoms reviving other phantoms. Default 60000 (60 seconds).', min: 0 },
            { key: 'PhantomHeroesExcludedRegions',        section: 'CustomGameOptions', type: 'string', label: 'Phantom Heroes Excluded Regions',       description: "Comma-separated substrings matched against each region's FULL prototype path (same matching style as RogueNemesisExcludedRegions/IncursionAllowedRegions). Phantoms cannot be spawned in a region matching one of these - use for content that scales around a fixed player count (e.g. wave-battle events), where extra phantoms per player would break the scaling. Empty by default." },
          ],
        },
        {
          title: 'Interactive Throwables Settings',
          fields: [
            { key: 'DisableInteractiveThrowables',   section: 'CustomGameOptions', type: 'bool', label: 'Disable Interactive Throwables',   description: 'Completely disables the ability for players to pick up and throw objects.' },
            { key: 'AutoCancelThrowableOnPowerUse',  section: 'CustomGameOptions', type: 'bool', label: 'Auto Cancel Throwable On Power Use', description: 'When true, attempting to use any other power while holding a throwable will silently drop the object and allow the new power to proceed.' },
            { key: 'AutoThrowOnMovementPower',       section: 'CustomGameOptions', type: 'bool', label: 'Auto Throw On Movement Power',      description: 'When true, activating a movement / travel power while holding a throwable will automatically throw it before starting movement.' },
          ],
        },
        {
          title: 'Chest Auto Open Settings',
          fields: [
            { key: 'EnableItemChestAutoOpen',                 section: 'CustomGameOptions', type: 'bool',   label: 'Enable Item Chest Auto Open',                description: 'Automatically opens chest items in the general inventory on a cooldown timer.' },
            { key: 'ItemChestAutoOpenCooldownMs',             section: 'CustomGameOptions', type: 'number', label: 'Item Chest Auto Open Cooldown (ms)',        description: 'Milliseconds between each automatic chest opening. Minimum 500 ms.', min: 500 },
            { key: 'ItemChestAutoOpenWhitelist',              section: 'CustomGameOptions', type: 'string', label: 'Item Chest Auto Open Whitelist',            description: 'Comma-separated list of prototype name substrings that identify chests to auto-open.' },
            { key: 'EnableItemChestAutoOpenVerboseLogging',   section: 'CustomGameOptions', type: 'bool',   label: 'Item Chest Auto Open Verbose Logging',      description: 'Logs verbose diagnostics for chest auto-open decisions. Off by default.' },
          ],
        },
        {
          title: 'Incursion Settings',
          fields: [
            { key: 'IncursionEnable',                        section: 'CustomGameOptions', type: 'bool',   label: 'Incursion Enable',                          description: 'Master switch for the experimental invasion system. When on, a hostile invader (the "Skrull Imposter") hunts one assigned player per eligible patrol zone on a timer.' },
            { key: 'IncursionIntervalMs',                    section: 'CustomGameOptions', type: 'number', label: 'Incursion Interval (ms)',                    description: 'Milliseconds between invasion waves. Minimum 1000 ms.', min: 1000 },
            { key: 'IncursionRandomIntervalMaxMs',           section: 'CustomGameOptions', type: 'number', label: 'Incursion Random Interval Max (ms)',        description: 'Maximum additional random milliseconds added to Incursion Interval for each wave.', min: 0 },
            { key: 'IncursionMaxActiveInvaders',             section: 'CustomGameOptions', type: 'number', label: 'Incursion Max Active Invaders',             description: 'Maximum number of invaders active at once.', min: 0 },
            { key: 'IncursionMaxLifetimeMs',                 section: 'CustomGameOptions', type: 'number', label: 'Incursion Max Lifetime (ms)',               description: 'Maximum lifetime of an invader before it is forcibly despawned.', min: 0 },
            { key: 'IncursionIdleTimeoutMs',                 section: 'CustomGameOptions', type: 'number', label: 'Incursion Idle Timeout (ms)',               description: 'How long an invader can go without engaging before it is despawned for being idle.', min: 0 },
            { key: 'IncursionDeathGracePeriodMs',            section: 'CustomGameOptions', type: 'number', label: 'Incursion Death Grace Period (ms)',         description: 'Controller kept alive this long after death because lingering DoTs still need to reference the damage scale.', min: 0 },
            { key: 'IncursionDeathVaporizeDelayMs',           section: 'CustomGameOptions', type: 'number', label: 'Incursion Death Vaporize Delay (ms)',      description: 'How long the body is visible in its "downed, awaiting resurrection" pose (the standard avatar death reaction) before it vaporizes and the death-reveal corpse appears. Lower = shorter kneel window, but too low may cut off the teleport-beam VFX before it reads clearly.', min: 0 },
            { key: 'IncursionEnemyDamageTakenMultiplier',    section: 'CustomGameOptions', type: 'number', label: 'Incursion Enemy Damage Taken Multiplier',   description: 'Incoming damage multiplier (goal = short lived but dangerous).', min: 0 },
            { key: 'IncursionEnemyVisualScale',              section: 'CustomGameOptions', type: 'number', label: 'Incursion Enemy Visual Scale',              description: 'Visual size/bounds multiplier at spawn (1.0=normal, 1.5=50% larger).', min: 0 },
            { key: 'IncursionEnemyDamageMultiplier',         section: 'CustomGameOptions', type: 'number', label: 'Incursion Enemy Damage Multiplier',         description: 'Final outgoing damage multiplier applied AFTER per-ability scale (1.0=no change, 1.5=+50%, 0.5=half).', min: 0 },
            { key: 'IncursionMaxSingleHitPctOfTargetHealth',section: 'CustomGameOptions', type: 'number', label: 'Incursion Max Single Hit % Of Target Health', description: "Hard ceiling on a single hit from an invader to a player, as a fraction of the target's max health, applied AFTER every other damage multiplier. 0.15 = no single hit can exceed 15% of the target's max health. 0 or negative disables the clamp." },
            { key: 'IncursionEnemyHealthMaxOverride',        section: 'CustomGameOptions', type: 'number', label: 'Incursion Enemy Health Max Override',       description: "Fixed HP pool given to the invader body, replacing whatever its native mob-curve produces once CombatLevel is force-synced to the target player's level. RogueNemesis rank multiplies this base value via RogueNemesisTiers.json's healthMult. 0 or negative disables the override and falls back to the native curve." },
            { key: 'IncursionExcludeEnemies',                section: 'CustomGameOptions', type: 'string', label: 'Incursion Exclude Enemies',                 description: 'Comma-separated list of invader chassis to exclude from random selection.' },
            { key: 'IncursionEnemyPrototype',                section: 'CustomGameOptions', type: 'string', label: 'Incursion Enemy Prototype',                 description: "Prototype name (full Calligraphy path) of the invader's COMBAT BODY (server-driven agent). Leave empty for the default proven boss. Playable avatars cannot be used." },
            { key: 'IncursionAllowedRegions',                section: 'CustomGameOptions', type: 'string', label: 'Incursion Allowed Regions',                 description: "Comma-separated substrings matched against each region's FULL prototype path (e.g. \"Regions/Sandbox/OriginalPatrols/Region1.prototype\"). The automatic hunt system ONLY spawns in regions whose path contains one of these (case-insensitive) - empty means it spawns nowhere. Does not affect manual !incursion commands. Does not require a rebuild to change, only a restart." },
            { key: 'IncursionRecentlyHuntedCooldownMs',      section: 'CustomGameOptions', type: 'number', label: 'Incursion Recently Hunted Cooldown (ms)',   description: 'After an invader defeats its assigned target, that player is skipped when picking a new zone target for this long (anti spawn-camping).', min: 0 },
            { key: 'IncursionDeathRevealMobPrototypes',      section: 'CustomGameOptions', type: 'string', label: 'Incursion Death Reveal Mob Prototypes',     description: 'Comma-separated full prototype paths of the "true form" corpse left behind where an invader just vaporized (the Skrull-reveal twist). One is picked at random per death.' },
            { key: 'IncursionCommandsRequireAdmin',          section: 'CustomGameOptions', type: 'bool',   label: 'Incursion Commands Require Admin',          description: "True restricts '!incursion' commands to admin accounts (server console is always allowed regardless of this setting)." },
            { key: 'IncursionLoggingEnable',                 section: 'CustomGameOptions', type: 'bool',   label: 'Incursion Logging Enable',                  description: 'Enables general Incursion logging.' },
            { key: 'IncursionLogVerboseEnable',               section: 'CustomGameOptions', type: 'bool',   label: 'Incursion Log Verbose Enable',              description: 'When true, logs detailed per-tick/per-spawn invasion diagnostics to help debug the system.' },
            { key: 'IncursionLogAllDamageTargetsEnable',      section: 'CustomGameOptions', type: 'bool',   label: 'Incursion Log All Damage Targets Enable',   description: 'When true, logs incursion enemy damage dealt to ALL targets (turrets, NPCs, etc.). When false (default), only logs damage dealt to player avatars for cleaner tuning output.' },
            { key: 'IncursionLogCollatorEnable',              section: 'CustomGameOptions', type: 'bool',   label: 'Incursion Log Collator Enable',             description: 'Saves per-incursion fight log to file.' },
          ],
        },
        {
          title: 'RogueNemesis Settings',
          fields: [
            { key: 'RogueNemesisEnable',                        section: 'CustomGameOptions', type: 'bool',   label: 'RogueNemesis Enable',                          description: "Master switch. Off by default, same \"off until you turn it on\" posture as Incursion. Players opt in individually with '!rogue enable' once turned on server-wide." },
            { key: 'RogueNemesisCheckIntervalMs',               section: 'CustomGameOptions', type: 'number', label: 'RogueNemesis Check Interval (ms)',            description: 'How often (ms) each opted-in eligible player gets an ambush roll.', min: 0 },
            { key: 'RogueNemesisRollChance',                    section: 'CustomGameOptions', type: 'number', label: 'RogueNemesis Roll Chance',                    description: 'Chance [0.0-1.0] of an ambush on each check for an eligible player.', min: 0, max: 1 },
            { key: 'RogueNemesisCooldownMs',                    section: 'CustomGameOptions', type: 'number', label: 'RogueNemesis Cooldown (ms)',                  description: 'After an encounter ends (all its invaders killed/despawned), how long before that player is eligible to be rolled again.', min: 0 },
            { key: 'RogueNemesisMaxSpawns',                     section: 'CustomGameOptions', type: 'number', label: 'RogueNemesis Max Spawns',                     description: 'How many invaders spawn together in a single encounter.', min: 0 },
            { key: 'RogueNemesisExcludedRegions',                section: 'CustomGameOptions', type: 'string', label: 'RogueNemesis Excluded Regions',                description: "Comma-separated substrings matched against each region's FULL prototype path (same matching style as IncursionAllowedRegions). Hubs/Town regions are ALWAYS excluded regardless of this setting - this is for additional zones you want off-limits on top of that, e.g. raid instances. Empty by default (everywhere but hubs is allowed)." },
            { key: 'RogueNemesisFollowDelayMs',                  section: 'CustomGameOptions', type: 'number', label: 'RogueNemesis Follow Delay (ms)',              description: 'When a hunted player changes to a non-safe zone, how long the invader takes to catch up and reappear off-screen in the new zone.', min: 0 },
            { key: 'RogueNemesisTargetUnreachableTimeoutMs',    section: 'CustomGameOptions', type: 'number', label: 'RogueNemesis Target Unreachable Timeout (ms)', description: "When the hunted player's entity can't be resolved in this Game instance at all (mid-region-transfer loading screen, OR they crossed into a different Game instance the invader has no way to follow into, e.g. a hub) - how long to wait before giving up and ending the hunt.", min: 0 },
            { key: 'RogueNemesisRankWeightMultiplier',           section: 'CustomGameOptions', type: 'number', label: 'RogueNemesis Rank Weight Multiplier',         description: "How much a player's existing Nemesis history biases which villain gets picked for their next encounter. Each Nemesis rank (1-5, still undefeated) adds this much extra weight relative to a fresh villain with no history - higher means an active Nemesis comes back far more often than a random rogue; 0 disables the bias entirely (uniform random pick, same as before Nemesis tracking existed).", min: 0 },
            { key: 'RogueNemesisCuratedRogueWeightShare',        section: 'CustomGameOptions', type: 'number', label: 'RogueNemesis Curated Rogue Weight Share',     description: "RoguesGallery.json's heroRogues/villainHunters curated lists (e.g. Thor -> Loki) are no longer the ENTIRE pool - they're unioned with the wider fallback pool, and this is the share of pick weight (0.0-1.0) the curated entries collectively get at baseline (no Nemesis rank history yet). 1.0 = old \"curated list only\" behavior; 0.0 = curated entries are effectively never picked.", min: 0, max: 1 },
            { key: 'RogueNemesisTier5DefeatCooldownEnable',      section: 'CustomGameOptions', type: 'bool',   label: 'RogueNemesis Tier 5 Defeat Cooldown Enable',  description: "After a player defeats a rank-5 (Tier 5) Nemesis, that specific villain won't spawn for them again until the next daily reset - stops endless same-day rank-5 loot farming without touching the fight itself or any other rank. Set false to disable entirely for testing." },
            { key: 'RogueNemesisTier5DefeatCooldownResetHour',   section: 'CustomGameOptions', type: 'number', label: 'RogueNemesis Tier 5 Defeat Cooldown Reset Hour', description: 'Local server-clock hour (0-23) the daily rank-5 defeat cooldown resets at.', min: 0, max: 23 },
            { key: 'RogueNemesisCommandsRequireAdmin',           section: 'CustomGameOptions', type: 'bool',   label: 'RogueNemesis Commands Require Admin',         description: "Gates admin/diagnostic '!rogue' subcommands only - the player-facing enable/disable toggle always works regardless of this setting." },
            { key: 'RogueNemesisLoggingEnable',                  section: 'CustomGameOptions', type: 'bool',   label: 'RogueNemesis Logging Enable',                 description: 'Enables general RogueNemesis logging.' },
            { key: 'RogueNemesisLogVerboseEnable',                section: 'CustomGameOptions', type: 'bool',   label: 'RogueNemesis Log Verbose Enable',             description: 'Enables detailed per-tick/per-encounter RogueNemesis diagnostics.' },
          ],
        },
        {
          title: 'Dinos Invade Manhattan / PvEScaleGameMode Settings',
          fields: [
            { key: 'DinosWaveBattleLoggingEnable', section: 'CustomGameOptions', type: 'bool', label: 'Dinos Wave Battle Logging Enable', description: 'Logs a structured per-run threat/kill/power-up/phase-transition trace to Logs/DinosWaveBattle/ for balance testing (e.g. modeling whether a given player count can clear the event). One file per continuous playthrough, flushed only when the run truly ends. Off by default.' },
            { key: 'DinosDisableBonusItemFind',     section: 'CustomGameOptions', type: 'bool', label: 'Dinos Disable Bonus Item Find',    description: 'Dinos Invade Manhattan generates far more kills per minute than any other content, which lets Bonus Item Find (BIF) accumulate at a rate well above what the rest of the game intends, effectively letting it be farmed. When true, forces this region\'s BonusItemFindMultiplier to 0. Off by default. Requires a restart to take effect.' },
          ],
        },
      ],
    },
    {
      id: 'MTXStore',
      label: 'MTXStore',
      fields: [
        { key: 'GazillioniteBalanceForNewAccounts', section: 'MTXStore', type: 'number', label: 'Gazillionite Balance For New Accounts', description: 'The amount of Gs new accounts will get when they log in for the first time.', min: 0 },
        { key: 'ESToGazillioniteConversionRatio',   section: 'MTXStore', type: 'number', label: 'ES To Gazillionite Conversion Ratio',   description: 'The amount of Gs players will get for each Eternity Splinter when converting them.' },
        { key: 'ESToGazillioniteConversionStep',    section: 'MTXStore', type: 'number', label: 'ES To Gazillionite Conversion Step',    description: 'Step for the Eternity Splinter amount when converting to G. Used to avoid rounding errors.', min: 1 },
        { key: 'GiftingOmegaLevelRequired',          section: 'MTXStore', type: 'number', label: 'Gifting Omega Level Required',          description: 'Minimum Omega level required to purchase gifts for other players (if Omega is enabled).', min: 0 },
        { key: 'GiftingInfinityLevelRequired',       section: 'MTXStore', type: 'number', label: 'Gifting Infinity Level Required',       description: 'Minimum Infinity level required to purchase gifts for other players (if Infinity is enabled).', min: 0 },
        { key: 'HomePageUrl',                        section: 'MTXStore', type: 'string', label: 'Home Page URL',                         description: 'URL for the page displayed on the Home tab.' },
        { key: 'HomeBannerPageUrl',                  section: 'MTXStore', type: 'string', label: 'Home Banner Page URL',                  description: 'URL for the top banner displayed when the Home tab is selected.' },
        { key: 'HeroesBannerPageUrl',                section: 'MTXStore', type: 'string', label: 'Heroes Banner Page URL',                description: 'URL for the top banner displayed when the Heroes tab is selected.' },
        { key: 'CostumesBannerPageUrl',               section: 'MTXStore', type: 'string', label: 'Costumes Banner Page URL',              description: 'URL for the top banner displayed when the Costumes tab is selected.' },
        { key: 'BoostsBannerPageUrl',                 section: 'MTXStore', type: 'string', label: 'Boosts Banner Page URL',                 description: 'URL for the top banner displayed when the Items tab is selected.' },
        { key: 'ChestsBannerPageUrl',                 section: 'MTXStore', type: 'string', label: 'Chests Banner Page URL',                 description: 'URL for the top banner displayed when the Cards tab is selected.' },
        { key: 'SpecialsBannerPageUrl',               section: 'MTXStore', type: 'string', label: 'Specials Banner Page URL',               description: 'URL for the top banner displayed when the Specials tab is selected.' },
        { key: 'RealMoneyUrl',                        section: 'MTXStore', type: 'string', label: 'Real Money URL',                         description: 'URL for the page displayed in the add G window.' },
        { key: 'RewriteOriginalBundleUrls',          section: 'MTXStore', type: 'bool',   label: 'Rewrite Original Bundle URLs',          description: 'When enabled, rewrites original bundle URLs pointing to the Gazillion CDN with new prefixes.' },
        { key: 'BundleInfoUrl',                       section: 'MTXStore', type: 'string', label: 'Bundle Info URL',                        description: 'URL used to rewrite Gazillion bundle info URLs (if enabled).' },
        { key: 'BundleImageUrl',                      section: 'MTXStore', type: 'string', label: 'Bundle Image URL',                       description: 'URL used to rewrite Gazillion bundle image URLs (if enabled).' },
      ],
    },
    {
      id: 'Leaderboards',
      label: 'Leaderboards',
      fields: [
        { key: 'DatabaseFile',            section: 'Leaderboards', type: 'string', label: 'Database File',             description: "Name of the SQLite database file to use relative to the server's Data/Leaderboards directory." },
        { key: 'ScheduleFile',            section: 'Leaderboards', type: 'string', label: 'Schedule File',             description: "Name of the JSON schedule file to use relative to the server's Data/Leaderboards directory." },
        { key: 'AutoSaveIntervalMinutes', section: 'Leaderboards', type: 'number', label: 'Autosave Interval (min)',   description: 'The minimum interval in minutes between leaderboard entry autosaves to the database.', min: 1 },
      ],
    },
    {
      id: 'multiverse',
      label: 'MH Multiverse',
    }
  ]

  // -- State --

  let activeSection = schema[0].id
  let values: IniData = {}
  let savedValues: IniData = {}
  let overridden: Record<string, string[]> = {}
  let loaded = false
  let loadError = ''
  let saveError = ''
  let saveSuccess = false
  let tooltip = ''
  let tooltipX = 0
  let tooltipY = 0
  let tooltipVisible = false
  let presets: string[] = []

  $: canLoad = !!$appConfig.server_exe
  $: activeSchema = schema.find(s => s.id === activeSection)!
  $: schemaFields = activeSchema.fields
    ?? activeSchema.subsections?.flatMap(s => s.fields)
    ?? []

  // Any key that exists in Config.ini/ConfigOverride.ini for this page's section
  // but isn't hand-described in the schema above -- so a new setting added
  // upstream shows up here immediately instead of silently staying invisible
  // until someone remembers to add it to the schema.
  $: knownKeys = new Set(schemaFields.map(f => f.key))
  $: unlistedFields = (activeSection === 'multiverse'
    ? []
    : Object.keys(values[activeSchema.label] ?? {})
        .filter(k => !knownKeys.has(k))
        .sort()
        .map(k => ({
          key: k,
          section: activeSchema.label,
          type: inferFieldType(values[activeSchema.label]?.[k] ?? ''),
          label: k,
          description: 'Not yet described in MH Multiverse -- found directly in Config.ini.',
        })))

  $: allFields = schemaFields.concat(unlistedFields)
  $: sections = (() => {
    const base = activeSchema.subsections
      ? activeSchema.subsections.map(s => ({ title: s.title, fields: s.fields }))
      : [{ title: '', fields: activeSchema.fields ?? [] }]
    if (unlistedFields.length > 0) {
      base.push({ title: 'Other settings (found in Config.ini)', fields: unlistedFields })
    }
    return base
  })()
  $: dirty = loaded && allFields.some(f =>
    (values[f.section]?.[f.key] ?? '') !== (savedValues[f.section]?.[f.key] ?? '')
  )
  $: presets = [...($appConfig.console_presets ?? [])]

  // -- Load / save --

  async function load() {
    loadError = ''
    try {
      const state = await invoke<{ values: IniData; overridden: Record<string, string[]> }>(
        'read_config', { serverExe: $appConfig.server_exe }
      )
      values = state.values
      savedValues = JSON.parse(JSON.stringify(state.values))
      overridden = state.overridden
      loaded = true
    } catch (e) {
      loadError = String(e)
    }
  }

  async function save() {
    saveError = ''
    saveSuccess = false
    const updates: IniData = {}
    for (const field of allFields) {
      const val = values[field.section]?.[field.key]
      if (val !== undefined) {
        if (!updates[field.section]) updates[field.section] = {}
        updates[field.section][field.key] = val
      }
    }
    try {
      await invoke('write_config', { serverExe: $appConfig.server_exe, updates })
      await load()
      saveSuccess = true
      setTimeout(() => saveSuccess = false, 3000)
    } catch (e) {
      saveError = String(e)
    }
  }

  async function resetSection() {
    saveError = ''
    const iniSections = [...new Set(allFields.map(f => f.section))]
    try {
      for (const section of iniSections) {
        await invoke('reset_config_section', { serverExe: $appConfig.server_exe, section })
      }
      await load()
    } catch (e) {
      saveError = String(e)
    }
  }

  function getValue(field: Field): string {
    return values[field.section]?.[field.key] ?? ''
  }

  function setValue(field: Field, val: string) {
    if (!values[field.section]) values[field.section] = {}
    values[field.section][field.key] = val
    values = { ...values }
  }

  function isOverridden(field: Field): boolean {
    return (overridden[field.section] ?? []).includes(field.key)
  }

  function isModified(field: Field): boolean {
    return (values[field.section]?.[field.key] ?? '') !== (savedValues[field.section]?.[field.key] ?? '')
  }

  function inferFieldType(value: string): FieldType {
    const trimmed = value.trim()
    const lower = trimmed.toLowerCase()
    if (lower === 'true' || lower === 'false') return 'bool'
    if (trimmed !== '' && !isNaN(Number(trimmed))) return 'number'
    return 'string'
  }

  // -- Tooltip --

  let tooltipEl: HTMLDivElement | null = null

  const TOOLTIP_OFFSET = 12
  const VIEWPORT_PAD = 10

  async function showTooltip(e: MouseEvent, text: string) {
    tooltip = text
    tooltipVisible = true

    await tick()
    if (!tooltipEl) return

    const rect = tooltipEl.getBoundingClientRect()
    const vw = window.innerWidth
    const vh = window.innerHeight

    let x = e.clientX + TOOLTIP_OFFSET
    let y = e.clientY - 8

    if (x + rect.width + VIEWPORT_PAD > vw) {
      x = e.clientX - rect.width - TOOLTIP_OFFSET
    }
    x = Math.max(VIEWPORT_PAD, Math.min(x, vw - rect.width - VIEWPORT_PAD))

    if (y < VIEWPORT_PAD) {
      y = e.clientY + TOOLTIP_OFFSET
    }
    y = Math.max(VIEWPORT_PAD, Math.min(y, vh - rect.height - VIEWPORT_PAD))

    tooltipX = x
    tooltipY = y
  }

  function hideTooltip() {
    tooltipVisible = false
  }

  // -- Console presets --

  function syncPresets() {
    setConsolePresets(presets)
  }

  function removePreset(i: number) {
    presets = presets.filter((_, idx) => idx !== i)
    syncPresets()
  }

  function movePresetUp(i: number) {
    if (i === 0) return
    const next = [...presets]
    ;[next[i - 1], next[i]] = [next[i], next[i - 1]]
    presets = next
    syncPresets()
  }

  function movePresetDown(i: number) {
    if (i === presets.length - 1) return
    const next = [...presets]
    ;[next[i], next[i + 1]] = [next[i + 1], next[i]]
    presets = next
    syncPresets()
  }

  // -- Preset autocomplete --

  interface Cmd { f: string; a: string; d: string; invokerType?: string }
  const presetCommands: Cmd[] = FALLBACK_COMMANDS.map(c => ({ ...c, invokerType: 'Any' }))

  let presetInput = ''
  let presetInputEl: HTMLInputElement | null = null
  let pacVisible = false
  let pacSuggs: Cmd[] = []
  let pacResolved: Cmd | null = null
  let pacSel = -1
  let pacFocused = false
  let pacPanelEl: HTMLElement | null = null

  $: if (pacSel >= 0 && pacPanelEl) {
    const selected = pacPanelEl.querySelector<HTMLElement>('.pac-sugg.sel')
    selected?.scrollIntoView({ block: 'nearest' })
  }

  function pacLcp(strs: string[]): string {
    if (!strs.length) return ''
    let p = strs[0]
    for (let i = 1; i < strs.length; i++) {
      while (!strs[i].startsWith(p)) p = p.slice(0, -1)
      if (!p) return ''
    }
    return p
  }

  function pacGetSuggs(v: string): Cmd[] {
    if (!v || v === '!') return []
    const lo = v.toLowerCase()
    return presetCommands.filter(c => c.f.startsWith(lo))
  }

  function pacFindResolved(v: string): Cmd | null {
    const lo = v.toLowerCase()
    return presetCommands.find(c => lo === c.f || lo.startsWith(c.f + ' ')) ?? null
  }

  function pacUpdate(v: string) {
    if (!v || v === '!') {
      pacVisible = false; pacSuggs = []; pacResolved = null; pacSel = -1
      return
    }
    const r = pacFindResolved(v)
    if (r) {
      pacResolved = r; pacSuggs = []; pacSel = -1; pacVisible = true
    } else {
      pacResolved = null; pacSel = -1; pacSuggs = pacGetSuggs(v); pacVisible = pacSuggs.length > 0
    }
  }

  function pacCompleteToCmd(c: Cmd) {
    presetInput = c.f + (c.a ? ' ' : '')
    pacSel = -1
    pacUpdate(presetInput)
  }

  function pacDoTab() {
    if (pacResolved) return
    if (!pacSuggs.length) return
    const prefix = pacLcp(pacSuggs.map(c => c.f))
    if (prefix.length > presetInput.length) {
      if (pacSuggs.length === 1) {
        pacCompleteToCmd(pacSuggs[0])
      } else {
        presetInput = prefix
        pacSel = -1
        pacUpdate(prefix)
      }
      return
    }
    if (pacSel === -1) {
      pacSel = 0
    } else {
      pacCompleteToCmd(pacSuggs[pacSel])
    }
  }

  function onPresetInputKeydown(e: KeyboardEvent) {
    if (e.key === 'Tab') {
      e.preventDefault()
      pacDoTab()
      return
    }
    if (e.key === 'Escape') {
      pacVisible = false; pacSel = -1; pacResolved = null
      return
    }
    if (e.key === 'Enter') {
      if (pacSel >= 0 && !pacResolved) {
        e.preventDefault()
        pacCompleteToCmd(pacSuggs[pacSel])
        return
      }
      addPreset()
      return
    }
    if (pacResolved || !pacVisible) return
    if (e.key === 'ArrowDown') { e.preventDefault(); pacSel = Math.min(pacSel + 1, pacSuggs.length - 1) }
    else if (e.key === 'ArrowUp') { e.preventDefault(); pacSel = Math.max(pacSel - 1, -1) }
  }

  function addPreset() {
    const v = presetInput.trim()
    if (!v) return
    presets = [...presets, v]
    presetInput = ''
    pacVisible = false; pacSuggs = []; pacResolved = null; pacSel = -1
    syncPresets()
  }

  // -- Preset drag reorder (pointer events) --
  //
  // HTML5 drag-and-drop is unreliable in Tauri WebViews. Instead we use pointer
  // events on the drag handle only: pointerdown starts the drag, pointermove and
  // pointerup are registered on window for the duration, and elementFromPoint
  // identifies the target chip under the cursor.

  let dragIndex: number | null = null
  let dragOverIndex: number | null = null

  function onHandlePointerDown(e: PointerEvent, i: number) {
    e.preventDefault()
    ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
    dragIndex = i
    dragOverIndex = i

    function onPointerMove(me: PointerEvent) {
      const el = document.elementFromPoint(me.clientX, me.clientY)
      const chip = el?.closest<HTMLElement>('[data-chip-index]')
      if (chip) {
        const idx = parseInt(chip.dataset.chipIndex ?? '', 10)
        if (!isNaN(idx)) dragOverIndex = idx
      }
    }

    function onPointerUp() {
      if (dragIndex !== null && dragOverIndex !== null && dragIndex !== dragOverIndex) {
        const next = [...presets]
        const [item] = next.splice(dragIndex, 1)
        next.splice(dragOverIndex, 0, item)
        presets = next
        syncPresets()
      }
      dragIndex = null
      dragOverIndex = null
      window.removeEventListener('pointermove', onPointerMove)
      window.removeEventListener('pointerup', onPointerUp)
    }

    window.addEventListener('pointermove', onPointerMove)
    window.addEventListener('pointerup', onPointerUp)
  }

  // -- Preset chip keyboard reorder --

  function onChipKeydown(e: KeyboardEvent, i: number) {
    if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
      e.preventDefault()
      if (i === 0) return
      movePresetUp(i)
      tick().then(() => {
        document.querySelectorAll<HTMLElement>('[data-chip-index]')[i - 1]?.focus()
      })
    } else if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
      e.preventDefault()
      if (i === presets.length - 1) return
      movePresetDown(i)
      tick().then(() => {
        document.querySelectorAll<HTMLElement>('[data-chip-index]')[i + 1]?.focus()
      })
    } else if (e.key === 'Delete' || e.key === 'Backspace') {
      e.preventDefault()
      removePreset(i)
      tick().then(() => {
        const chips = document.querySelectorAll<HTMLElement>('[data-chip-index]')
        if (chips.length) {
          chips[Math.min(i, chips.length - 1)]?.focus()
        } else {
          presetInputEl?.focus()
        }
      })
    }
  }

  // --

  onMount(() => {
    if (canLoad) load()
  })
</script>

{#if tooltipVisible}
  <div
    bind:this={tooltipEl}
    class="tooltip"
    style="left:{tooltipX}px; top:{tooltipY}px"
  >
    {tooltip}
  </div>
{/if}

<div class="config-panel">
  <div class="panel-bg"></div>
  <div class="grid-overlay"></div>
  <div class="config-layout">

  <!-- Left nav -->
  <PanelSidebar width="var(--sidebar-narrow)">
    <svelte:fragment slot="header">
      {#if onBack}
        <button class="btn-icon" on:click={onBack} title="Back" style="margin-right:4px;">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="15 18 9 12 15 6"/>
          </svg>
        </button>
      {/if}
      {#if !embedded}
        <div class="section-title">Config</div>
      {/if}

      <button
        class="btn-icon"
        on:click={openConfigDir}
        title="Open MHServerEmu folder"
        disabled={!$appConfig.server_exe}
        style="margin-left:auto;"
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
        </svg>
      </button>
    </svelte:fragment>
    <div class="config-nav-list">
      {#each schema as section}
        <button
          class="config-nav-item"
          class:active={activeSection === section.id}
          on:click={() => activeSection = section.id}
        >
          {section.label}
        </button>
      {/each}
    </div>
  </PanelSidebar>

  <!-- Main content -->
  <div class="config-main">

    {#if !canLoad && activeSection !== 'multiverse'}
      <div class="config-notice">
        <div class="notice-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:20px;height:20px;color:var(--text-3)">
            <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/>
            <circle cx="12" cy="12" r="3"/>
          </svg>
        </div>
        <div class="notice-text">Set the server executable path in App settings to load configuration.</div>
      </div>
    {:else if activeSection === 'multiverse'}
      <div class="config-content">
        <div class="config-section-head">
          <div class="section-title">MH Multiverse</div>
        </div>
        <div class="config-body">
          <div class="multiverse-note">
            These settings are stored in MH Multiverse, not in ConfigOverride.ini.
          </div>

          <div class="subsection-title">Shutdown</div>
          <div class="config-grid">
            <div class="config-field">
              <div class="config-field-head">
                <span class="config-field-label">Delay (min)</span>
              </div>
              <input
                type="number"
                class="config-input"
                min="0"
                value={$appConfig.shutdown.delay_minutes}
                on:change={(e) => setShutdownConfig({
                  ...$appConfig.shutdown,
                  delay_minutes: Math.max(0, parseInt(e.currentTarget.value) || 0)
                })}
              >
              <span class="config-field-hint">0 = stop immediately with no broadcast</span>
            </div>
            <div class="config-field">
              <div class="config-field-head">
                <span class="config-field-label">Broadcast Message</span>
              </div>
              <input
                type="text"
                class="config-input"
                value={$appConfig.shutdown.broadcast_message}
                placeholder="Server is shutting down in {'{minutes}'} minute(s)."
                on:change={(e) => setShutdownConfig({
                  ...$appConfig.shutdown,
                  broadcast_message: e.currentTarget.value
                })}
              >
              <span class="config-field-hint">Use {'{minutes}'} as a placeholder for the remaining time.</span>
            </div>
          </div>

          <div class="subsection-title">Console</div>
          <p class="subsection-desc">
            Saved commands shown as quick-access chips in the console.
            Drag by the handle or use arrow keys on a focused chip to reorder. Delete or Backspace removes it.
          </p>

          {#if presets.length > 0}
            <div class="preset-chips" role="list" aria-label="Console presets">
              {#each presets as preset, i}
                <div
                  class="preset-chip"
                  class:is-dragging={dragIndex === i}
                  class:is-drag-over={dragOverIndex === i && dragIndex !== null && dragIndex !== i}
                  data-chip-index={i}
                  tabindex="0"
                  role="button"
                  aria-label="{preset}, position {i + 1} of {presets.length}. Arrow keys to reorder, Delete to remove."
                  on:keydown={(e) => onChipKeydown(e, i)}
                >
                  <svg
                    class="preset-chip-handle"
                    viewBox="0 0 8 14"
                    fill="currentColor"
                    aria-hidden="true"
                    on:pointerdown={(e) => onHandlePointerDown(e, i)}
                  >
                    <circle cx="2" cy="2"  r="1.2"/>
                    <circle cx="6" cy="2"  r="1.2"/>
                    <circle cx="2" cy="7"  r="1.2"/>
                    <circle cx="6" cy="7"  r="1.2"/>
                    <circle cx="2" cy="12" r="1.2"/>
                    <circle cx="6" cy="12" r="1.2"/>
                  </svg>
                  <span class="preset-chip-text">{preset}</span>
                  <button
                    class="preset-chip-remove"
                    tabindex="-1"
                    aria-label="Remove {preset}"
                    on:click|stopPropagation={() => removePreset(i)}
                  >
                    <svg viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
                      <line x1="2" y1="2" x2="8" y2="8"/>
                      <line x1="8" y1="2" x2="2" y2="8"/>
                    </svg>
                  </button>
                </div>
              {/each}
            </div>
          {:else}
            <div class="preset-empty">No presets yet</div>
          {/if}

          <div class="preset-add-wrap">
            {#if pacVisible && pacFocused}
              <div class="pac-panel" bind:this={pacPanelEl}>
                <div class="pac-header">
                  <span class="pac-hint">TAB to complete &nbsp;|&nbsp; ↑↓ navigate &nbsp;|&nbsp; ESC dismiss</span>
                  {#if !pacResolved}
                    <span class="pac-count">{pacSuggs.length} match{pacSuggs.length !== 1 ? 'es' : ''}</span>
                  {/if}
                </div>
                {#if pacResolved}
                  <div class="pac-resolved">
                    <div class="pac-resolved-cmd">
                      <span class="pac-resolved-base">{pacResolved.f}</span>
                      {#if pacResolved.a}<span class="pac-resolved-args"> {pacResolved.a}</span>{/if}
                    </div>
                    <div class="pac-resolved-desc">{pacResolved.d}</div>
                  </div>
                {:else}
                  {#each pacSuggs as c, i (c.f)}
                    <div
                      class="pac-sugg"
                      class:sel={i === pacSel}
                      on:mousedown|preventDefault={() => pacCompleteToCmd(c)}
                      role="option"
                      tabindex="-1"
                      aria-selected={i === pacSel}
                    >
                      <div class="pac-sugg-top">
                        <span class="pac-sugg-cmd">
                          <span class="pac-typed">{c.f.slice(0, presetInput.length)}</span>{c.f.slice(presetInput.length)}
                        </span>
                        {#if c.a}<span class="pac-sugg-args"> {c.a}</span>{/if}
                      </div>
                      <div class="pac-sugg-desc">{c.d}</div>
                    </div>
                  {/each}
                {/if}
              </div>
            {/if}
            <div class="preset-add-row">
              <span class="preset-add-prefix">&gt;</span>
              <input
                type="text"
                class="preset-add-input"
                bind:value={presetInput}
                bind:this={presetInputEl}
                placeholder="Type a command..."
                on:input={() => pacUpdate(presetInput)}
                on:keydown={onPresetInputKeydown}
                on:focus={() => { pacFocused = true; if (presetInput) pacUpdate(presetInput) }}
                on:blur={() => setTimeout(() => { pacFocused = false; pacVisible = false }, 150)}
              >
              <button class="btn btn-sm btn-accent" on:click={addPreset}>Add</button>
            </div>
          </div>

        <div class="config-field full">
          <div class="toggle-row" style="margin-bottom: 8px;">
              <div class="toggle-info">
                <span class="config-field-label">Threshold Log Filter</span>
                <span class="config-tooltip">When on, the log filter shows the selected level and all more severe levels. When off, only the exact selected level is shown.</span>
              </div>
              <div
                class="toggle-switch"
                class:on={$logFilterThreshold}
                role="switch"
                aria-checked={$logFilterThreshold}
                tabindex="0"
                on:click={() => logFilterThreshold.update(v => !v)}
                on:keydown={(e) => e.key === 'Enter' && logFilterThreshold.update(v => !v)}
              ></div>
            </div>
        </div>

        </div>
      </div>

    {:else if !loaded}
      <div class="config-notice">
        <div class="notice-text">Config not loaded yet.</div>
        {#if loadError}
          <div class="notice-error">{loadError}</div>
        {/if}
        <button class="btn btn-accent" on:click={load}>Load Config</button>
      </div>
    {:else}

      <div class="config-content">
        {#if $serverRunning}
          <div class="running-warn">
            Server is currently running -- changes will take effect on next restart.
          </div>
        {/if}

        <div class="config-section-head">
          <div class="section-title">{activeSchema.label}</div>
        </div>

        <div class="config-body">
          {#each sections as section}
            {#if section.title}
              <div class="subsection-title">{section.title}</div>
            {/if}

            <div class="config-grid">
              {#each section.fields as field}
                {#if field.type === 'bool'}
                  <div class="config-field full">
                    <div class="toggle-row">
                      <div class="toggle-info">
                        <span class="config-field-label">{field.label}</span>
                        <span class="config-tooltip">{field.description}</span>
                      </div>
                      <div
                        class="toggle-switch"
                        class:on={getValue(field).toLowerCase() === 'true'}
                        on:click={() => setValue(field, getValue(field).toLowerCase() === 'true' ? 'false' : 'true')}
                        role="switch"
                        aria-checked={getValue(field).toLowerCase() === 'true'}
                        tabindex="0"
                        on:keydown={(e) => e.key === 'Enter' && setValue(field, getValue(field).toLowerCase() === 'true' ? 'false' : 'true')}
                      ></div>
                    </div>
                  </div>
                {:else if field.type === 'textarea'}
                  <div class="config-field full">
                    <div class="config-field-head">
                      <span class="config-field-label">{field.label}</span>
                      {#if isOverridden(field)}
                        <span class="override-dot" title="Overridden from default"></span>
                      {/if}
                      <span class="config-field-default">
                        <button
                          class="info-btn"
                          on:mouseenter={(e) => showTooltip(e, field.description)}
                          on:mouseleave={hideTooltip}
                          tabindex="-1"
                        >?</button>
                      </span>
                    </div>
                    <textarea
                      value={getValue(field)}
                      on:change={(e) => setValue(field, e.currentTarget.value)}
                      rows="3"
                      class:modified={isModified(field)}
                    ></textarea>
                  </div>
                {:else}
                  <div class="config-field">
                    <div class="config-field-head">
                      <span class="config-field-label">{field.label}</span>
                      {#if isOverridden(field)}
                        <span class="override-dot" title="Overridden from default"></span>
                      {/if}
                      <span class="config-field-default">
                        <button
                          class="info-btn"
                          on:mouseenter={(e) => showTooltip(e, field.description)}
                          on:mouseleave={hideTooltip}
                          tabindex="-1"
                        >?</button>
                      </span>
                    </div>
                    {#if field.type === 'number'}
                      <input
                        type="number"
                        class="config-input"
                        class:modified={isModified(field)}
                        value={getValue(field)}
                        min={field.min}
                        max={field.max}
                        on:change={(e) => setValue(field, e.currentTarget.value)}
                      >
                    {:else}
                      <input
                        type="text"
                        class="config-input"
                        class:modified={isModified(field)}
                        value={getValue(field)}
                        on:change={(e) => setValue(field, e.currentTarget.value)}
                      >
                    {/if}
                  </div>
                {/if}
              {/each}
            </div>
          {/each}
        </div>
      </div>

      <!-- Footer -->
      <div class="panel-footer">
        {#if dirty}
          <span class="dirty-badge">Unsaved changes</span>
        {/if}
        {#if saveError}
          <span class="feedback-error">{saveError}</span>
        {/if}
        {#if saveSuccess}
          <span class="feedback-ok">Saved</span>
        {/if}
        <button class="btn btn-sm btn-outline" style="margin-left:auto;" on:click={resetSection} disabled={!loaded}>Reset to Defaults</button>
        <button class="btn btn-sm btn-accent" class:btn-pulse={dirty} on:click={save} disabled={!loaded}>Save</button>
      </div>

    {/if}
  </div>
  </div><!-- config-layout -->
</div>

<style>
  .config-panel {
    display: flex;
    flex: 1;
    flex-direction: column;
    position: relative;
    overflow: hidden;
    min-height: 0;
  }

  .config-layout {
    position: relative;
    z-index: 1;
    flex: 1;
    display: flex;
    overflow: hidden;
    min-height: 0;
  }

  .config-nav-list {
    padding: 6px;
  }

  .config-nav-item {
    display: block;
    width: 100%;
    text-align: left;
    padding: 10px 12px;
    font-family: var(--font-head);
    font-size: 13px;
    font-weight: 600;
    color: var(--text-1);
    background: none;
    border: 1px solid transparent;
    cursor: pointer;
    border-radius: var(--radius-sm);
    transition: all 0.12s;
    margin-bottom: 2px;
  }
  .config-nav-item:hover { color: var(--text-0); background: var(--bg-3); border-color: var(--border-mid); }
  .config-nav-item.active { color: var(--accent-bright); background: var(--accent-glow); border-color: var(--accent-dim); }

  /* -- Main area -- */
  .config-main {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    min-height: 0;
    background: var(--bg-1);
  }

  .config-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .config-section-head {
    display: flex;
    align-items: center;
    padding: 12px 20px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    min-height: 53px;
  }

  .config-body {
    flex: 1;
    overflow-y: auto;
    padding: 16px 20px;
  }

  .config-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
    margin-bottom: 20px;
  }

  .config-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .config-field.full { grid-column: 1 / -1; }

  .config-field-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .config-field-label {
    font-family: var(--font-head);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-1);
  }

  .config-field-default {
    margin-left: auto;
  }

  .config-input {
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    color: var(--text-0);
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 7px 10px;
    outline: none;
    border-radius: var(--radius-sm);
    transition: border-color 0.15s;
    width: 100%;
  }
  .config-input:focus { border-color: var(--accent-dim); }
  .config-input.modified {
    border-color: var(--accent-dim);
    background: var(--accent-glow);
  }

  .config-tooltip {
    font-size: 11px;
    color: var(--text-3);
    line-height: 1.4;
  }

  /* -- Toggle rows (booleans) -- */
  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
  }
  .toggle-row:last-child { border-bottom: none; }

  .toggle-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  /* -- Override dot -- */
  .override-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    flex-shrink: 0;
  }

  /* -- Info button -- */
  .info-btn {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 1px solid var(--border-lit);
    background: var(--bg-3);
    color: var(--text-3);
    font-size: 9px;
    cursor: help;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    transition: all 0.12s;
    padding: 0;
  }
  .info-btn:hover { border-color: var(--accent-dim); color: var(--accent-bright); }

  /* -- Textarea -- */
  textarea {
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    color: var(--text-0);
    font-family: var(--font-body);
    font-size: 12px;
    padding: 7px 10px;
    outline: none;
    width: 100%;
    resize: vertical;
    border-radius: var(--radius-sm);
    transition: border-color 0.15s;
  }
  textarea:focus { border-color: var(--accent-dim); }
  textarea.modified {
    border-color: var(--accent-dim);
    background: var(--accent-glow);
  }

  /* -- Notices -- */
  .config-notice {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    padding: 24px;
  }

  .notice-text {
    font-family: var(--font-head);
    font-size: 12px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
  }

  .notice-error {
    font-size: 12px;
    color: var(--text-error);
  }

  .running-warn {
    padding: 8px 20px;
    background: var(--amber-dim);
    border-bottom: 1px solid rgba(200, 146, 10, 0.2);
    font-size: 12px;
    color: var(--amber);
    font-family: var(--font-head);
    letter-spacing: 0.06em;
    flex-shrink: 0;
  }

  .multiverse-note {
    font-size: 11px;
    color: var(--text-3);
    margin-bottom: 14px;
    font-family: var(--font-body);
  }

  .config-field-hint {
    font-size: 10px;
    color: var(--text-3);
    font-family: var(--font-body);
    margin-top: 2px;
  }

  /* -- Tooltip -- */
  .tooltip {
    position: fixed;
    z-index: var(--z-tooltip);
    background: var(--bg-3);
    border: 1px solid var(--border-lit);
    border-radius: var(--radius-sm);
    color: var(--text-1);
    font-size: 12px;
    font-family: var(--font-body);
    padding: 6px 10px;
    max-width: 300px;
    line-height: 1.5;
    pointer-events: none;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
  }

  /* -- Subsection description -- */
  .subsection-desc {
    font-size: 11px;
    color: var(--text-3);
    line-height: 1.5;
    margin: -8px 0 14px;
    font-family: var(--font-body);
  }

  /* -- Preset chips -- */
  .preset-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 12px;
  }

  .preset-chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 6px 4px 5px;
    background: var(--bg-3);
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-1);
    user-select: none;
    transition: border-color 0.12s, background 0.12s, opacity 0.12s;
    outline: none;
  }
  .preset-chip:focus {
    border-color: var(--accent-dim);
    background: var(--accent-glow);
    box-shadow: 0 0 0 2px var(--accent-glow);
  }
  .preset-chip.is-dragging {
    opacity: 0.4;
  }
  .preset-chip.is-drag-over {
    border-color: var(--accent-dim);
    background: var(--accent-glow);
  }

  .preset-chip-handle {
    width: 8px;
    height: 14px;
    color: var(--text-3);
    flex-shrink: 0;
    cursor: grab;
    touch-action: none;
  }
  .preset-chip-handle:active {
    cursor: grabbing;
  }

  .preset-chip-text {
    line-height: 1;
  }

  .preset-chip-remove {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    padding: 0;
    background: none;
    border: none;
    color: var(--text-3);
    cursor: pointer;
    border-radius: 2px;
    flex-shrink: 0;
    transition: color 0.1s, background 0.1s;
  }
  .preset-chip-remove:hover {
    color: var(--text-error);
    background: var(--red-dim);
  }
  .preset-chip-remove svg {
    width: 10px;
    height: 10px;
  }

  .preset-empty {
    font-size: 11px;
    color: var(--text-3);
    font-family: var(--font-head);
    letter-spacing: 0.06em;
    margin-bottom: 12px;
    padding: 10px 0;
  }

  /* -- Preset add row -- */
  .preset-add-wrap {
    position: relative;
    margin-bottom: 10px;
  }

  .preset-add-row {
    display: flex;
    align-items: center;
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    background: var(--bg-0);
    overflow: hidden;
    transition: border-color 0.15s;
  }
  .preset-add-wrap:focus-within .preset-add-row {
    border-color: var(--accent-dim);
  }

  .preset-add-prefix {
    padding: 0 0 0 10px;
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--accent-dim);
    user-select: none;
    flex-shrink: 0;
  }

  .preset-add-input {
    flex: 1;
    background: none;
    border: none;
    color: var(--text-0);
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 7px 8px;
    outline: none;
  }
  .preset-add-input::placeholder { color: var(--text-3); }

  .preset-add-row .btn {
    margin: 4px;
    flex-shrink: 0;
  }

  /* -- Preset autocomplete panel -- */
  .pac-panel {
    position: absolute;
    bottom: 100%;
    left: 0;
    right: 0;
    margin-bottom: 4px;
    background: var(--bg-2);
    border: 1px solid var(--border-lit);
    border-radius: var(--radius-sm);
    max-height: 240px;
    overflow-y: auto;
    z-index: var(--z-dropdown);
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.35);
  }
  .pac-panel::-webkit-scrollbar { width: 4px; }
  .pac-panel::-webkit-scrollbar-thumb { background: var(--border-lit); }

  .pac-header {
    padding: 5px 12px;
    background: var(--bg-0);
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-shrink: 0;
  }
  .pac-hint {
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .pac-count {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-3);
  }

  .pac-sugg {
    padding: 7px 12px;
    cursor: pointer;
    border-bottom: 1px solid var(--border);
    border-left: 2px solid transparent;
    transition: background 0.08s;
  }
  .pac-sugg:last-child { border-bottom: none; }
  .pac-sugg:hover { background: var(--accent-glow); }
  .pac-sugg.sel { background: var(--accent-glow); border-left-color: var(--accent-dim); }

  .pac-sugg-top { display: flex; align-items: baseline; gap: 6px; }
  .pac-sugg-cmd { font-family: var(--font-mono); font-size: 12px; color: var(--text-1); }
  .pac-typed { color: var(--accent-bright); }
  .pac-sugg-args { font-family: var(--font-mono); font-size: 12px; color: var(--text-3); }
  .pac-sugg-desc { font-size: 11px; color: var(--text-3); margin-top: 3px; line-height: 1.4; }

  .pac-resolved { padding: 8px 12px; }
  .pac-resolved-cmd { font-family: var(--font-mono); font-size: 12px; }
  .pac-resolved-base { color: var(--green-bright); }
  .pac-resolved-args { color: var(--text-3); }
  .pac-resolved-desc { font-size: 11px; color: var(--text-3); margin-top: 3px; }
</style>