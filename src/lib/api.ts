/**
 * Typed bindings for the Rust commands.
 *
 * Under `tauri dev` these hit the real backend. Opened in a plain browser
 * (`npm run dev`) there is no IPC bridge, so every call is served by the mock in
 * `mock.ts` — that keeps the UI workable without hardware or a Rust build.
 */
import type {
  BatteryEvent,
  Config,
  Device,
  DeviceListPayload,
  DeviceProfile,
  DpiState,
  FeatureInfo,
  LightingRequest,
  ReportRateState,
  Settings,
  ZoneInfo,
  Application,
  ApplicationCommands,
  DatabaseInfo,
  ArtworkLayout,
  GhubCacheInfo,
  ImportedDevice,
  ImportReport,
  CommunityIndex,
  SharedProfile,
  Game,
  WheelState,
  WheelSettings,
} from "./types";

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

type Handler = (payload: unknown) => void;

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri) {
    const { mockInvoke } = await import("./mock");
    return mockInvoke<T>(command, args ?? {});
  }
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}

/** Subscribes to a backend event; resolves to an unsubscribe function. */
export async function on<T>(event: string, handler: (payload: T) => void): Promise<() => void> {
  if (!isTauri) {
    const { mockListen } = await import("./mock");
    return mockListen(event, handler as Handler);
  }
  const { listen } = await import("@tauri-apps/api/event");
  const unlisten = await listen<T>(event, (e) => handler(e.payload));
  return unlisten;
}

export const events = {
  devicesChanged: "devices-changed",
  batteryUpdate: "battery-update",
  deviceUpdated: "device-updated",
  activeApplication: "active-application",
  configChanged: "config-changed",
  artworkChanged: "artwork-changed",
  wheelState: "wheel-state",
} as const;

// -- devices ---------------------------------------------------------------

export const getConnectedDevices = (refresh = true) =>
  call<DeviceListPayload>("get_connected_devices", { refresh });

export const getDeviceState = (deviceId: string) =>
  call<Device>("get_device_state", { deviceId });

export const getDeviceFeatures = (deviceId: string) =>
  call<FeatureInfo[]>("get_device_features", { deviceId });

export const setDeviceDpi = (deviceId: string, dpi: number) =>
  call<DpiState>("set_device_dpi", { deviceId, dpi });

export const setPollingRate = (deviceId: string, rateHz: number) =>
  call<ReportRateState>("set_polling_rate", { deviceId, rateHz });

export const setDeviceLighting = (request: LightingRequest) =>
  call<void>("set_device_lighting", { request });

export const getLightingZones = (deviceId: string) =>
  call<ZoneInfo[]>("get_lighting_zones", { deviceId });

// -- application database --------------------------------------------------

export const getApplications = () => call<Application[]>("get_applications");
export const getApplicationCommands = (applicationId: string) =>
  call<ApplicationCommands>("get_application_commands", { applicationId });
export const getApplicationDatabaseInfo = () =>
  call<DatabaseInfo | null>("get_application_database_info");
export const refreshApplicationDatabase = () =>
  call<DatabaseInfo>("refresh_application_database");
export const getActiveApplication = () => call<string | null>("get_active_application");
export const bindProfileApplication = (profileId: string, applicationId: string | null) =>
  call<Config>("bind_profile_application", { profileId, applicationId });
export const setProfileDisabled = (profileId: string, disabled: boolean) =>
  call<Config>("set_profile_disabled", { profileId, disabled });

// -- community profiles ----------------------------------------------------

export const getCommunityIndex = (refresh = false) =>
  call<CommunityIndex>("get_community_index", { refresh });
export const previewCommunityProfile = (path: string) =>
  call<SharedProfile>("preview_community_profile", { path });
export const importCommunityProfile = (shared: SharedProfile) =>
  call<Config>("import_community_profile", { shared });
export const exportProfile = (profileId: string, deviceId: string, description: string) =>
  call<string>("export_profile", { profileId, deviceId, description });
export const writeTextFile = (path: string, contents: string) =>
  call<void>("write_text_file", { path, contents });

// -- onboard profiles ------------------------------------------------------

/** Writes macro bindings to the device. Returns the backup path taken first. */
export const applyOnboardMacros = (
  deviceId: string,
  assignments: { button: number; steps: import("$lib/macros").MacroStep[] }[],
) => call<string>("apply_onboard_macros", { deviceId, assignments });

export const backupOnboardMemory = (deviceId: string) =>
  call<string>("backup_onboard_memory", { deviceId });

export const restoreOnboardMemory = (deviceId: string, path: string) =>
  call<number>("restore_onboard_memory", { deviceId, path });

export const readBatteries = () => call<BatteryEvent[]>("read_batteries");

export const setDemoMode = (enabled: boolean) =>
  call<DeviceListPayload>("set_demo_mode", { enabled });

// -- profiles & settings ---------------------------------------------------

export const getConfig = () => call<Config>("get_config");
export const saveConfig = (config: Config) => call<Config>("save_config", { config });
export const setActiveProfile = (profileId: string) =>
  call<Config>("set_active_profile", { profileId });
export const createProfile = (name: string, kind = "game") =>
  call<Config>("create_profile", { name, kind });
export const deleteProfile = (profileId: string) =>
  call<Config>("delete_profile", { profileId });
export const saveDeviceProfile = (deviceId: string, profile: DeviceProfile) =>
  call<Config>("save_device_profile", { deviceId, profile });
export const getDeviceProfile = (deviceId: string) =>
  call<DeviceProfile>("get_device_profile", { deviceId });
export const saveSettings = (settings: Settings) => call<Config>("save_settings", { settings });
export const getConfigPath = () => call<string>("get_config_path");

// -- steering wheels -------------------------------------------------------

export const getWheelState = (deviceId: string) => call<WheelState>("get_wheel_state", { deviceId });
export const getWheelSettings = (deviceId: string) =>
  call<WheelSettings>("get_wheel_settings", { deviceId });
export const setWheelSettings = (deviceId: string, settings: WheelSettings) =>
  call<WheelSettings>("set_wheel_settings", { deviceId, settings });
export const setWheelLeds = (deviceId: string, mask: number) =>
  call<void>("set_wheel_leds", { deviceId, mask });
export const calibrateWheelCenter = (deviceId: string, maxDegrees = 10) =>
  call<WheelSettings>("calibrate_wheel_center", { deviceId, maxDegrees });
export const resetWheelCenter = (deviceId: string) =>
  call<WheelSettings>("reset_wheel_center", { deviceId });
export const setWheelDriver = (enabled: boolean) => call<Config>("set_wheel_driver", { enabled });

// -- games library ---------------------------------------------------------

export const getGames = (refresh = false) => call<Game[]>("get_games", { refresh });
export const launchGame = (gameId: string) => call<void>("launch_game", { gameId });
export const addManualGame = (name: string, exec: string, args: string, cover: string | null) =>
  call<Config>("add_manual_game", { name, exec, args, cover });
export const removeManualGame = (id: string) => call<Config>("remove_manual_game", { id });

// -- artwork ---------------------------------------------------------------

export const getArtwork = () => call<Record<string, string>>("get_artwork");
export const getArtworkDir = () => call<string>("get_artwork_dir");
export const getArtworkLayout = (productIds: number[]) =>
  call<ArtworkLayout | null>("get_artwork_layout", { productIds });

// -- G HUB depots ----------------------------------------------------------

export const importGhubProgramData = (path: string) =>
  call<ImportReport>("import_ghub_program_data", { path });
export const fetchDeviceArtwork = (deviceId: string) =>
  call<ImportedDevice>("fetch_device_artwork", { deviceId });
export const getGhubCacheInfo = () => call<GhubCacheInfo | null>("get_ghub_cache_info");

// -- window ----------------------------------------------------------------

export const windowMinimize = () => call<void>("window_minimize");
export const windowToggleMaximize = () => call<boolean>("window_toggle_maximize");
export const windowClose = () => call<void>("window_close");

/** Normalises a rejected IPC promise into a readable message. */
export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}
