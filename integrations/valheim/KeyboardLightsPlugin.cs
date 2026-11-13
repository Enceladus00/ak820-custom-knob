// Sends what happens to the local player to ak820-custom-knob, which lights
// the keyboard to match: the biome's colour, a boss fight's dark pulse,
// thunderstorm flashes, fire while burning, a heartbeat on low health, and
// flashes when you take or land a hit, kill, parry or level up.
//
// Protocol (see crates/knob-core/src/game_light.rs): one UDP datagram of text
// to 127.0.0.1:<port>. State lines whenever the state changes and at least
// every 0.5 s; event lines once, the moment something happens:
//
//   knob-game 1 valheim health=0.85 dead=0 effects=burning biome=swamp night=1 weather=ThunderStorm boss=0
//   knob-game 1 valheim event=hurt amount=0.30
//
// The plugin only reads the game. Its Harmony patches watch combat methods
// without changing what they do, and every one of them swallows its own
// errors, so a bug here can't break a fight. It needs no other players to
// have it and is safe on any server.

using System;
using System.Collections.Generic;
using System.Globalization;
using System.Net.Sockets;
using System.Reflection;
using System.Text;
using BepInEx;
using BepInEx.Configuration;
using HarmonyLib;
using UnityEngine;

namespace AK820KeyboardLights
{
    [BepInPlugin(Guid, "AK820 Keyboard Lights", "1.1.0")]
    public class KeyboardLightsPlugin : BaseUnityPlugin
    {
        public const string Guid = "ak820customknob.valheim.keyboardlights";

        const float CheckEvery = 0.1f;
        const float SendAtLeastEvery = 0.5f;
        const float BossScanEvery = 0.5f;
        /// <summary>A boss this close and fighting counts as a boss fight.</summary>
        const float BossRange = 80f;

        static readonly (string Name, Func<int> Hash)[] Effects =
        {
            ("burning", () => SEMan.s_statusEffectBurning),
            ("lightning", () => SEMan.s_statusEffectLightning),
            ("poison", () => SEMan.s_statusEffectPoison),
            ("frost", () => SEMan.s_statusEffectFrost),
            ("freezing", () => SEMan.s_statusEffectFreezing),
            ("resting", () => SEMan.s_statusEffectResting),
            ("cold", () => SEMan.s_statusEffectCold),
            ("wet", () => SEMan.s_statusEffectWet),
            ("smoked", () => SEMan.s_statusEffectSmoked),
        };

        static KeyboardLightsPlugin instance;

        ConfigEntry<int> port;
        UdpClient udp;
        Harmony harmony;
        float nextCheck;
        float nextBossScan;
        bool bossFight;
        float lastSent = float.NegativeInfinity;
        string lastLine;
        float lastPowerCooldown = float.NaN;
        bool warned;

        void Awake()
        {
            instance = this;
            port = Config.Bind("General", "Port", 50820,
                "UDP port ak820-custom-knob listens on (game_lighting.port in its config).");
            udp = new UdpClient();
            harmony = new Harmony(Guid);
            harmony.PatchAll(typeof(Patches));
            Logger.LogInfo($"sending player state to 127.0.0.1:{port.Value}");
        }

        void OnDestroy()
        {
            harmony?.UnpatchSelf();
            udp?.Dispose();
            if (instance == this) instance = null;
        }

        void Update()
        {
            float now = Time.unscaledTime;
            if (now < nextCheck) return;
            nextCheck = now + CheckEvery;

            try
            {
                Player player = Player.m_localPlayer;
                if (player != null && now >= nextBossScan)
                {
                    nextBossScan = now + BossScanEvery;
                    bossFight = BossFighting(player);
                }
                WatchPower(player);

                string line = Describe(player, bossFight);
                if (line == lastLine && now - lastSent < SendAtLeastEvery) return;
                lastLine = line;
                lastSent = now;
                Send(line);
            }
            catch (Exception e)
            {
                WarnOnce($"cannot read the player: {e.Message}");
            }
        }

        void Send(string line)
        {
            try
            {
                byte[] bytes = Encoding.UTF8.GetBytes(line);
                udp.Send(bytes, bytes.Length, "127.0.0.1", port.Value);
            }
            catch (Exception e)
            {
                // Nobody listening is normal; warn once about anything else.
                WarnOnce($"cannot send player state: {e.Message}");
            }
        }

        void WarnOnce(string message)
        {
            if (warned) return;
            warned = true;
            Logger.LogWarning(message);
        }

        static string Number(float value) => value.ToString("0.00", CultureInfo.InvariantCulture);

        /// <summary>Sends a moment such as a hit. Called from the patches.</summary>
        internal static void SendEvent(string name, float amount)
        {
            instance?.Send($"knob-game 1 valheim event={name} amount={Number(Mathf.Clamp01(amount))}");
        }

        /// <summary>Notices the Forsaken power being used and coming back.</summary>
        void WatchPower(Player player)
        {
            if (player == null)
            {
                lastPowerCooldown = float.NaN;
                return;
            }
            player.GetGuardianPowerHUD(out StatusEffect power, out float cooldown);
            if (power == null)
            {
                lastPowerCooldown = float.NaN;
                return;
            }
            if (!float.IsNaN(lastPowerCooldown))
            {
                if (lastPowerCooldown > 0f && cooldown <= 0f) SendEvent("power_ready", 1f);
                else if (cooldown > lastPowerCooldown + 1f) SendEvent("power_used", 1f);
            }
            lastPowerCooldown = cooldown;
        }

        static bool BossFighting(Player player)
        {
            Vector3 here = player.transform.position;
            foreach (Character c in Character.GetAllCharacters())
            {
                if (c == null || !c.IsBoss() || c.IsDead()) continue;
                if (Vector3.Distance(c.transform.position, here) > BossRange) continue;
                BaseAI ai = c.GetBaseAI();
                if (ai == null || ai.IsAlerted() || ai.HaveTarget()) return true;
            }
            return false;
        }

        static string Describe(Player player, bool bossFight)
        {
            if (player == null) return "knob-game 1 valheim";

            var effects = new List<string>();
            SEMan seman = player.GetSEMan();
            if (seman != null)
            {
                foreach (var (name, hash) in Effects)
                {
                    if (seman.HaveStatusEffect(hash())) effects.Add(name);
                }
            }
            var line = new StringBuilder("knob-game 1 valheim");
            // Invariant culture: a Turkish or German Windows would write 0,85.
            line.Append(" health=").Append(Number(player.GetHealthPercentage()));
            line.Append(" dead=").Append(player.IsDead() ? '1' : '0');
            line.Append(" effects=").Append(string.Join(",", effects));
            line.Append(" biome=").Append(player.GetCurrentBiome().ToString().ToLowerInvariant());
            line.Append(" night=").Append(EnvMan.IsNight() ? '1' : '0');
            string weather = EnvMan.instance?.GetCurrentEnvironment()?.m_name;
            if (!string.IsNullOrEmpty(weather)) line.Append(" weather=").Append(weather.Replace(" ", ""));
            line.Append(" boss=").Append(bossFight ? '1' : '0');
            return line.ToString();
        }
    }

    /// <summary>
    /// Watches combat. Each patch only reads, and catches its own errors so the
    /// game's code always runs as normal.
    /// </summary>
    static class Patches
    {
        /// <summary>Damage over time has its own lighting (fire, poison...), so its ticks don't flash.</summary>
        static readonly HashSet<HitData.HitType> Ticks = new HashSet<HitData.HitType>
        {
            HitData.HitType.Burning,
            HitData.HitType.Poisoned,
            HitData.HitType.Freezing,
            HitData.HitType.Smoke,
            HitData.HitType.Drowning,
        };

        /// <summary>The perfect-block window, read from the game; 0.25 s if it can't be found.</summary>
        static readonly float PerfectBlockWindow = ReadPerfectBlockWindow();

        static int lastHitFrame = -1;

        static float ReadPerfectBlockWindow()
        {
            try
            {
                FieldInfo field = AccessTools.Field(typeof(Humanoid), "m_perfectBlockInterval");
                if (field?.GetValue(null) is float value && value > 0f) return value;
            }
            catch
            {
            }
            return 0.25f;
        }

        static bool IsLocal(Character c) => c != null && c == Player.m_localPlayer;

        static float Share(float damage, Character of) => damage / Mathf.Max(of.GetMaxHealth(), 1f);

        [HarmonyPatch(typeof(Player), "OnDamaged")]
        [HarmonyPostfix]
        static void Hurt(Player __instance, HitData hit)
        {
            try
            {
                if (!IsLocal(__instance) || hit == null || Ticks.Contains(hit.m_hitType)) return;
                float damage = hit.GetTotalDamage();
                if (damage < 0.1f) return;
                KeyboardLightsPlugin.SendEvent("hurt", Share(damage, __instance));
            }
            catch
            {
            }
        }

        [HarmonyPatch(typeof(Character), nameof(Character.Damage))]
        [HarmonyPrefix]
        static void Hit(Character __instance, HitData hit)
        {
            try
            {
                // One flash per frame, even when a swing hits a whole pack.
                if (hit == null || IsLocal(__instance) || Time.frameCount == lastHitFrame) return;
                if (!IsLocal(hit.GetAttacker())) return;
                lastHitFrame = Time.frameCount;
                KeyboardLightsPlugin.SendEvent("hit", Share(hit.GetTotalDamage(), __instance));
            }
            catch
            {
            }
        }

        [HarmonyPatch(typeof(Character), nameof(Character.OnDeath))]
        [HarmonyPostfix]
        static void Killed(Character __instance, bool ___m_localPlayerHasHit)
        {
            try
            {
                if (__instance is Player || !___m_localPlayerHasHit) return;
                KeyboardLightsPlugin.SendEvent(__instance.IsBoss() ? "bosskill" : "kill", 1f);
            }
            catch
            {
            }
        }

        [HarmonyPatch(typeof(Humanoid), "BlockAttack")]
        [HarmonyPrefix]
        static void BeforeBlock(Humanoid __instance, float ___m_blockTimer, out bool __state)
        {
            __state = false;
            try
            {
                __state = IsLocal(__instance) && ___m_blockTimer >= 0f && ___m_blockTimer < PerfectBlockWindow;
            }
            catch
            {
            }
        }

        [HarmonyPatch(typeof(Humanoid), "BlockAttack")]
        [HarmonyPostfix]
        static void AfterBlock(bool __result, bool __state)
        {
            if (__result && __state) KeyboardLightsPlugin.SendEvent("parry", 1f);
        }

        [HarmonyPatch(typeof(Player), nameof(Player.OnSkillLevelup))]
        [HarmonyPostfix]
        static void LevelUp(Player __instance)
        {
            try
            {
                if (IsLocal(__instance)) KeyboardLightsPlugin.SendEvent("levelup", 1f);
            }
            catch
            {
            }
        }
    }
}
