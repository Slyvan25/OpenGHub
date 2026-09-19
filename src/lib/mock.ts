/**
 * Browser-only stand-in for the Rust backend.
 *
 * Only used when the app is opened outside Tauri (`npm run dev` in a browser),
 * so the UI can be built and reviewed without hardware. Mirrors the shape of the
 * real commands exactly — nothing here is reachable from the packaged app.
 */
import type { Config, Device, DeviceProfile, LightingRequest } from "./types";

const devices: Device[] = [
  makeDevice("demo-g502", "G502 LIGHTSPEED", "mouse", 0x407f, {
    connection: "receiver",
    battery: 74,
    dpi: { current: 1600, min: 100, max: 25600, step: 50 },
    rates: [125, 250, 500, 1000],
    zones: 2,
    onboard: true,
  }),
  makeDevice("demo-prox60", "PRO X 60", "keyboard", 0x4097, {
    connection: "wireless",
    battery: 100,
    rates: [125, 250, 500, 1000],
    zones: 1,
    onboard: true,
  }),
  makeDevice("demo-prox2", "PRO X 2 LIGHTSPEED", "headset", 0x0afe, {
    connection: "wireless",
    battery: 17,
  }),
  makeDevice("demo-a50x", "A50 X Party Time", "headset", 0x0b02, {
    connection: "wireless",
    battery: 100,
  }),
  makeDevice("demo-litra-1", "LITRA BEAM", "light", 0xc901, { zones: 1 }),
  makeDevice("demo-litra-2", "LITRA BEAM", "light", 0xc901, { zones: 1 }),
  makeDevice("demo-yeti-gx", "Yeti GX", "microphone", 0x0ade, { zones: 1 }),
  makeDevice("demo-g923", "G923 Racing Wheel", "wheel", 0xc267, { wheel: true }),
];

interface Options {
  connection?: Device["connection"];
  battery?: number;
  dpi?: { current: number; min: number; max: number; step: number };
  rates?: number[];
  zones?: number;
  onboard?: boolean;
  wheel?: boolean;
}

function makeDevice(
  id: string,
  name: string,
  kind: Device["kind"],
  productId: number,
  o: Options,
): Device {
  // Range sensors are described by min/max/step, with no enumerated list —
  // the same shape the Rust side produces from feature 0x2201.
  const steps: number[] = [];
  return {
    id,
    name,
    kind,
    vendorId: 0x046d,
    productId,
    serial: null,
    modelIds: [productId],
    connection: o.connection ?? "wired",
    online: true,
    capabilities: {
      dpi: !!o.dpi,
      reportRate: !!o.rates,
      battery: o.battery !== undefined,
      lighting: !!o.zones,
      onboardMemory: !!o.onboard,
      wheel: !!o.wheel,
    },
    battery:
      o.battery === undefined
        ? null
        : {
            percentage: o.battery,
            approximate: false,
            status: o.battery === 100 ? "full" : "discharging",
            voltageMv: null,
          },
    dpi: o.dpi
      ? { ...o.dpi, sensor: 0, default: o.dpi.current, steps }
      : null,
    reportRate: o.rates
      ? { currentHz: 1000, availableHz: o.rates, extended: false }
      : null,
    lightingZones: o.zones ?? 0,
    onboardMode: o.onboard ? false : null,
    firmware: o.onboard ? [{ kind: "main", version: "MPM17.00_B0008", active: true }, { kind: "bootloader", version: "BOT92.00_B0008", active: false }] : [],
    wheel: o.wheel
      ? { rangeMin: 40, rangeMax: 900, rpmLeds: 5, protocol: "ClassicReport30", driverRunning: true, hardwareCalibration: false }
      : null,
    protocolVersion: o.wheel ? "classic" : "4.5",
    demo: true,
    lastError: null,
  };
}

const STORAGE_KEY = "openghub.mock.config";

function loadConfig(): Config {
  const stored = typeof localStorage !== "undefined" ? localStorage.getItem(STORAGE_KEY) : null;
  if (stored) {
    try {
      return JSON.parse(stored) as Config;
    } catch {
      /* fall through to defaults */
    }
  }
  return {
    profiles: [
      {
        id: "default",
        name: "Desktop: Default",
        kind: "desktop",
        devices: {
          // A recorded macro so the editor has something to show outside Tauri.
          "demo-g502": {
            dpiStages: [],
            activeStage: 0,
            reportRateHz: null,
            lighting: null,
            lightingZones: {},
            zonePositions: {},
            zoneNames: [],
            assignments: [],
            macros: [
              {
                id: "m-test",
                name: "test",
                kind: "noRepeat",
                useStandardDelays: true,
                standardDelayMs: 50,
                steps: [
                  { step: "keyDown", usage: 0x09 }, { step: "keyUp", usage: 0x09 },
                  { step: "keyDown", usage: 0x08 }, { step: "keyUp", usage: 0x08 },
                  { step: "keyDown", usage: 0x04 }, { step: "keyUp", usage: 0x04 },
                  { step: "keyDown", usage: 0x16 }, { step: "keyUp", usage: 0x16 },
                ],
              },
            ],
          },
        },
      },
    ],
    activeProfile: "default",
    settings: {
      startMinimised: false,
      showBatteryNotifications: true,
      batteryPollSeconds: 60,
      illuminationFollowsProfile: false,
      autoSwitchProfiles: true,
      autoFetchArtwork: true,
      persistentProfile: "default",
      communityRepo: "https://raw.githubusercontent.com/Slyvan25/openghub-community/main",
      authorName: "",
    },
  };
}

let config = loadConfig();

const scriptLog: string[] = [];
const stamp = () => new Date().toTimeString().slice(0, 8);

const mockDeviceSettings = {
  autoSleepMin: 0,
  inactivityLightingMin: 0,
  lowBatteryMode: false,
  lowBatteryThreshold: 15,
  lowBatteryBrightness: 20,
  leftHanded: false,
};

const mockWheel = {
  rangeDeg: 900,
  sensitivity: 50,
  centerSpring: 20,
  centerSpringInFfbGames: false,
  ffbGain: 100,
  centerOffset: 0,
  trueforceTorque: 100,
  trueforceAudio: 80,
  trueforceGameControl: true,
  gamepadMode: false,
  pedals: {
    accelerator: { sensitivity: 50, deadZoneLow: 0, deadZoneHigh: 0, inverted: false },
    brake: { sensitivity: 50, deadZoneLow: 0, deadZoneHigh: 0, inverted: false },
    clutch: { sensitivity: 50, deadZoneLow: 0, deadZoneHigh: 0, inverted: false },
    combined: false,
  },
};

function persist(): Config {
  if (typeof localStorage !== "undefined") {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(config));
  }
  return structuredClone(config);
}

const listeners = new Map<string, Set<(payload: unknown) => void>>();

export function mockListen(event: string, handler: (payload: unknown) => void): () => void {
  if (!listeners.has(event)) listeners.set(event, new Set());
  listeners.get(event)!.add(handler);
  return () => listeners.get(event)?.delete(handler);
}

function emit(event: string, payload: unknown) {
  listeners.get(event)?.forEach((fn) => fn(payload));
}

function device(id: string): Device {
  const found = devices.find((d) => d.id === id);
  if (!found) throw `device ${id} is not connected`;
  return found;
}

function activeProfile() {
  return config.profiles.find((p) => p.id === config.activeProfile) ?? config.profiles[0];
}

export async function mockInvoke<T>(command: string, args: Record<string, unknown>): Promise<T> {
  // A touch of latency, so loading states are exercised during UI work.
  await new Promise((r) => setTimeout(r, 40));

  switch (command) {
    case "get_connected_devices":
      return { devices: structuredClone(devices), demo: true, error: null, emptyReason: null } as T;

    case "get_device_state":
      return structuredClone(device(args.deviceId as string)) as T;

    case "get_device_features":
      return [
        { id: "0x0000", name: "Root", index: 0, obsolete: false, hidden: false },
        { id: "0x0001", name: "Feature Set", index: 1, obsolete: false, hidden: false },
        { id: "0x1004", name: "Unified Battery", index: 6, obsolete: false, hidden: false },
        { id: "0x2201", name: "Adjustable DPI", index: 9, obsolete: false, hidden: false },
        { id: "0x8060", name: "Adjustable Report Rate", index: 11, obsolete: false, hidden: false },
      ] as T;

    case "set_device_dpi": {
      const d = device(args.deviceId as string);
      if (!d.dpi) throw "device has no adjustable sensor";
      const requested = args.dpi as number;
      d.dpi.current = Math.min(Math.max(requested, d.dpi.min), d.dpi.max);
      emit("device-updated", structuredClone(d));
      return structuredClone(d.dpi) as T;
    }

    case "set_polling_rate": {
      const d = device(args.deviceId as string);
      if (!d.reportRate) throw "device has a fixed report rate";
      const hz = args.rateHz as number;
      d.reportRate.currentHz = d.reportRate.availableHz.reduce((a, b) =>
        Math.abs(b - hz) < Math.abs(a - hz) ? b : a,
      );
      emit("device-updated", structuredClone(d));
      return structuredClone(d.reportRate) as T;
    }

    case "set_device_lighting": {
      const request = args.request as LightingRequest;
      device(request.deviceId);
      return undefined as T;
    }

    case "read_batteries": {
      const events = devices
        .filter((d) => d.battery)
        .map((d) => {
          d.battery!.percentage =
            d.battery!.status === "full"
              ? d.battery!.percentage
              : Math.max(1, d.battery!.percentage - 1);
          return { deviceId: d.id, battery: structuredClone(d.battery!) };
        });
      events.forEach((e) => emit("battery-update", e));
      return events as T;
    }

    case "set_demo_mode":
      return { devices: structuredClone(devices), demo: true, error: null, emptyReason: null } as T;

    case "get_config":
      return structuredClone(config) as T;

    case "save_config":
      config = JSON.parse(JSON.stringify(args.config)) as Config;
      return persist() as T;

    case "set_active_profile":
      config.activeProfile = args.profileId as string;
      return persist() as T;

    case "create_profile": {
      const id = `p${Date.now()}`;
      config.profiles.push({
        id,
        name: args.name as string,
        kind: (args.kind as "game") ?? "game",
        devices: {},
      });
      config.activeProfile = id;
      return persist() as T;
    }

    case "delete_profile": {
      const id = args.profileId as string;
      if (id === "default") throw "the default profile cannot be deleted";
      config.profiles = config.profiles.filter((p) => p.id !== id);
      if (config.activeProfile === id) config.activeProfile = "default";
      return persist() as T;
    }

    case "save_device_profile":
      // Svelte state proxies cannot be structuredClone'd; store plain data.
      activeProfile().devices[args.deviceId as string] = JSON.parse(JSON.stringify(args.profile)) as DeviceProfile;
      return persist() as T;

    case "get_device_profile":
      return (activeProfile().devices[args.deviceId as string] ?? {
        dpiStages: [],
        activeStage: 0,
        reportRateHz: null,
        lighting: null,
        lightingZones: {},
        zonePositions: {},
        zoneNames: [],
        assignments: [],
        macros: [],
      }) as T;

    case "get_lighting_zones": {
      const d = device(args.deviceId as string);
      return Array.from({ length: d.lightingZones }, (_, index) => ({
        index,
        location: index + 1,
        locationName: index === 0 ? "Primary" : index === 1 ? "Logo" : `Zone ${index + 1}`,
        effects: [0x00, 0x01, 0x03, 0x0a],
      })) as T;
    }

    case "save_settings":
      config.settings = JSON.parse(JSON.stringify(args.settings)) as Config["settings"];
      return persist() as T;

    case "apply_onboard_macros":
      return "~/.local/share/openghub/backups/mock.json" as T;

    case "backup_onboard_memory":
      return "~/.local/share/openghub/backups/mock.json" as T;

    case "rename_profile": {
      const p = config.profiles.find((x) => x.id === args.profileId);
      if (p) p.name = args.name as string;
      return persist() as T;
    }
    case "duplicate_profile": {
      const src = config.profiles.find((x) => x.id === args.profileId);
      if (src) {
        const id = `p${Date.now()}`;
        config.profiles.push({ ...structuredClone(src), id, name: `${src.name} Copy`, disabled: false });
        config.activeProfile = id;
      }
      return persist() as T;
    }

    case "set_profile_script": {
      const p = config.profiles.find((x) => x.id === args.profileId);
      if (p) p.script = (args.script as string | null) ?? null;
      if (p && config.activeProfile === p.id) {
        scriptLog.length = 0;
        if (p.script) {
          scriptLog.push(`[${stamp()}] script started`, "PROFILE_ACTIVATED 0");
        }
      }
      return persist() as T;
    }
    case "get_script_log":
      return [...scriptLog] as T;
    case "clear_script_log":
      scriptLog.length = 0;
      return undefined as T;
    case "get_script_status": {
      const p = config.profiles.find((x) => x.id === config.activeProfile);
      const running = !!p?.script;
      return {
        running,
        profileId: running ? p!.id : null,
        onboardDevices: devices.filter((d) => d.onboardMode).map((d) => d.name),
      } as T;
    }

    case "get_device_settings":
      return { ...mockDeviceSettings } as T;
    case "set_device_settings":
      Object.assign(mockDeviceSettings, args.settings as object);
      return { ...mockDeviceSettings } as T;

    case "set_onboard_mode": {
      const d = device(args.deviceId as string);
      d.onboardMode = args.on as boolean;
      return structuredClone(d) as T;
    }
    case "import_ghub_settings":
      throw "importing G HUB settings needs the desktop app";

    case "set_zone_software_effect":
      return undefined as T;
    case "get_lightsync_status":
      return { activeZones: 1, error: null, screenAuthorised: true } as T;

    case "apply_assignments":
      return { software: true, onboard: (args.deviceId as string) === "demo-g502" } as T;

    case "get_wheel_state": {
      const t = Date.now() / 1000;
      return {
        steeringRaw: 32768 + Math.round(Math.sin(t) * 20000),
        steering: Math.sin(t) * 0.6,
        accelerator: (Math.sin(t * 1.3) + 1) / 2,
        brake: 0,
        clutch: 0,
        buttons: 0,
        hat: 8,
      } as T;
    }
    case "get_wheel_settings":
      return mockWheel as T;
    case "set_wheel_settings":
      Object.assign(mockWheel, args.settings as object);
      return { ...mockWheel } as T;
    case "calibrate_wheel_center":
      mockWheel.centerOffset = 120;
      return { ...mockWheel } as T;
    case "reset_wheel_center":
      mockWheel.centerOffset = 0;
      return { ...mockWheel } as T;
    case "set_wheel_leds":
      return undefined as T;
    case "set_wheel_driver":
      config.settings.wheelDriver = args.enabled as boolean;
      return persist() as T;

    case "get_games":
      return [
        { id: "steam:730", source: "steam", name: "Counter-Strike 2", cover: null, coverUrl: null, lastPlayed: 1789303750, playtimeMinutes: 10567, installDir: "/games/cs2", applicationId: "cs2" },
        { id: "steam:252950", source: "steam", name: "Rocket League", cover: null, coverUrl: null, lastPlayed: 1789303837, playtimeMinutes: 9365, installDir: null, applicationId: null },
        { id: "steam:945360", source: "steam", name: "Among Us", cover: null, coverUrl: null, lastPlayed: 1773784509, playtimeMinutes: 1563, installDir: null, applicationId: null },
        { id: "lutris:ubisoft-connect", source: "lutris", name: "Ubisoft Connect", cover: null, coverUrl: null, lastPlayed: 1720214050, playtimeMinutes: 598, installDir: null, applicationId: null },
        { id: "epic:Fortnite", source: "epic", name: "Fortnite", cover: null, coverUrl: null, lastPlayed: 0, playtimeMinutes: 0, installDir: null, applicationId: null },
        { id: "gog:1207658924", source: "gog", name: "The Witcher 3", cover: null, coverUrl: null, lastPlayed: 0, playtimeMinutes: 0, installDir: null, applicationId: null },
        { id: "manual:tempest-rising-1", source: "manual", name: "Tempest Rising", cover: null, coverUrl: null, lastPlayed: 0, playtimeMinutes: 0, installDir: "/games/tempest", applicationId: null },
      ] as T;

    case "launch_game":
      return undefined as T;

    case "add_manual_game":
    case "remove_manual_game":
      throw "managing games needs the desktop app";

    case "get_applications":
      return [
        { id: "cs2", name: "Counter-Strike 2", posterUrl: null, steamAppIds: ["730"], executables: [], commandCount: 33 },
        { id: "apex", name: "Apex Legends", posterUrl: null, steamAppIds: ["1172470"], executables: [], commandCount: 21 },
        { id: "valorant", name: "Valorant", posterUrl: null, steamAppIds: [], executables: ["valorant.exe"], commandCount: 18 },
      ] as T;

    case "get_application_commands":
      return {
        id: args.applicationId,
        name: "Counter-Strike 2",
        commands: [
          { category: "Movement", name: "Jump", keystroke: ["Space"] },
          { category: "Movement", name: "Crouch", keystroke: ["Ctrl"] },
          { category: "Weapons", name: "Reload", keystroke: ["R"] },
          { category: "Communications", name: "Voice chat", keystroke: ["K"] },
        ],
        categoryColors: [
          { hex: "#e42121", tag: "Movement" },
          { hex: "#2dd22f", tag: "Weapons" },
          { hex: "#9b24db", tag: "Communications" },
        ],
      } as T;

    case "get_application_database_info":
      return { version: "mock", applicationCount: 3, fetchedAt: 0, cachePath: "(mock)" } as T;

    case "refresh_application_database":
      return { version: "mock", applicationCount: 3, fetchedAt: Date.now() / 1000, cachePath: "(mock)" } as T;

    case "get_active_application":
      return null as T;

    case "get_community_index":
      return {
        version: 1,
        generated: "2026-09-13",
        profiles: [
          { id: "cs2-competitive-g502-wireless", name: "Counter-Strike 2: Competitive", author: "silvan",
            description: "400/800/1600, logo off, DPI shift on G9.", device: { modelId: "g502_wireless", productIds: [0x407f, 0xc08d], displayName: "G502 LIGHTSPEED", kind: "mouse" },
            application: { id: "cs2", name: "Counter-Strike 2" }, path: "profiles/g502_wireless/cs2-competitive.json",
            macroCount: 1, assignmentCount: 3, dpiStages: [400, 800, 1600], hasLighting: true, updated: "2026-09-13" },
          { id: "desktop-calm-g502-wireless", name: "Desktop: Calm", author: "ada",
            description: "Slow cyan breathing, 800 DPI.", device: { modelId: "g502_wireless", productIds: [0x407f], displayName: "G502 LIGHTSPEED", kind: "mouse" },
            application: null, path: "profiles/g502_wireless/desktop-calm.json",
            macroCount: 0, assignmentCount: 0, dpiStages: [800], hasLighting: true, updated: "2026-09-12" },
        ],
      } as T;

    case "preview_community_profile":
      return {
        format: 1, id: "cs2-competitive-g502-wireless", name: "Counter-Strike 2: Competitive", author: "silvan",
        description: "400/800/1600, logo off, DPI shift on G9.", license: "CC0-1.0",
        device: { modelId: "g502_wireless", productIds: [0x407f, 0xc08d], displayName: "G502 LIGHTSPEED", kind: "mouse" },
        application: { id: "cs2", name: "Counter-Strike 2" },
        profile: { dpiStages: [400, 800, 1600], activeStage: 1, reportRateHz: 1000, lighting: null,
          lightingZones: { "0": { effect: "fixed", color: "#00a9e0", brightness: 100, rateMs: 5000 }, "1": { effect: "off", color: "#000000", brightness: 0, rateMs: 5000 } },
          zonePositions: {}, zoneNames: ["Primary", "Logo"],
          assignments: [{ control: "button-9", category: "macro", label: "Buy AK", value: "m1" }],
          macros: [{ id: "m1", name: "Buy AK", steps: [{ step: "keyDown", usage: 0x05 }, { step: "keyUp", usage: 0x05 }, { step: "delay", ms: 40 }, { step: "keyDown", usage: 0x1f }, { step: "keyUp", usage: 0x1f }] }] },
      } as T;

    case "import_community_profile": {
      const s = args.shared as { name: string; application: { id: string } | null; profile: unknown };
      config.profiles.push({ id: `c${Date.now()}`, name: s.name, kind: s.application ? "game" : "app",
        applicationId: s.application?.id ?? null, devices: { "demo-g502": s.profile as never } } as never);
      return persist() as T;
    }

    case "export_profile":
      return JSON.stringify({ format: 1, id: "mock", name: "Mock" }, null, 2) as T;

    case "write_text_file":
      return undefined as T;
    case "read_text_file":
      return "" as T;

    case "set_profile_disabled": {
      const p = config.profiles.find((x) => x.id === args.profileId);
      if (p) p.disabled = args.disabled as boolean;
      return persist() as T;
    }

    case "bind_profile_application": {
      const p = config.profiles.find((x) => x.id === args.profileId);
      if (p) {
        p.applicationId = args.applicationId as string | null;
        if (args.applicationId && !p.name.includes(":")) p.name = `Game: ${p.name}`;
        p.kind = args.applicationId ? "game" : "desktop";
      }
      return persist() as T;
    }

    case "get_artwork_layout": {
      const ids = args.productIds as number[];
      for (const id of ids) {
        const key = id.toString(16).padStart(4, "0");
        const r = await fetch(`/__mock-artwork/${key}.layout.json`).catch(() => null);
        if (r?.ok) return (await r.json()) as T;
      }
      return null as T;
    }
    case "get_ghub_cache_info":
      return null as T;

    case "import_ghub_program_data":
      throw "importing G HUB data needs the desktop app";

    case "fetch_device_artwork":
      throw "fetching artwork needs the desktop app";

    case "get_artwork": {
      // The dev server exposes this user's fetched artwork (see vite.config.js).
      const r = await fetch("/__mock-artwork/index.json").catch(() => null);
      const files: string[] = r?.ok ? await r.json() : [];
      const map: Record<string, string> = {};
      for (const f of files) {
        const stem = f.replace(/\.(png|webp|jpe?g|json)$/, "");
        map[stem] = `/__mock-artwork/${f}`;
      }
      return map as T;
    }

    case "get_artwork_dir":
      return "~/.local/share/OpenGHub/devices (mock — running outside Tauri)" as T;

    case "get_config_path":
      return "~/.config/openghub/config.json (mock — running outside Tauri)" as T;

    case "window_minimize":
    case "window_close":
      return undefined as T;

    case "window_toggle_maximize":
      return false as T;

    default:
      throw `unknown command '${command}'`;
  }
}
