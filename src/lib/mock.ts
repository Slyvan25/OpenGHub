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
];

interface Options {
  connection?: Device["connection"];
  battery?: number;
  dpi?: { current: number; min: number; max: number; step: number };
  rates?: number[];
  zones?: number;
  onboard?: boolean;
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
    protocolVersion: "4.5",
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
    profiles: [{ id: "default", name: "Desktop: Default", kind: "desktop", devices: {} }],
    activeProfile: "default",
    settings: {
      startMinimised: false,
      showBatteryNotifications: true,
      batteryPollSeconds: 60,
      illuminationFollowsProfile: false,
    },
  };
}

let config = loadConfig();

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
      config = args.config as Config;
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
      activeProfile().devices[args.deviceId as string] = args.profile as DeviceProfile;
      return persist() as T;

    case "get_device_profile":
      return (activeProfile().devices[args.deviceId as string] ?? {
        dpiStages: [],
        activeStage: 0,
        reportRateHz: null,
        lighting: null,
        lightingZones: {},
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
      config.settings = args.settings as Config["settings"];
      return persist() as T;

    case "apply_onboard_macros":
      return "~/.local/share/openghub/backups/mock.json" as T;

    case "backup_onboard_memory":
      return "~/.local/share/openghub/backups/mock.json" as T;

    case "get_artwork":
      return {} as T;

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
