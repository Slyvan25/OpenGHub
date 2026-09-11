/** Mirrors the serde payloads in `src-tauri/src/state.rs` and `profiles.rs`. */

export type DeviceKind =
  | "mouse"
  | "keyboard"
  | "headset"
  | "speaker"
  | "microphone"
  | "light"
  | "webcam"
  | "wheel"
  | "receiver"
  | "other";

export type Connection = "wired" | "wireless" | "receiver" | "bluetooth";

export type ChargeStatus =
  | "discharging"
  | "charging"
  | "chargingFull"
  | "full"
  | "slowCharging"
  | "error"
  | "unknown";

export interface BatteryState {
  percentage: number;
  approximate: boolean;
  status: ChargeStatus;
  voltageMv: number | null;
}

export interface DpiState {
  sensor: number;
  current: number;
  default: number;
  steps: number[];
  min: number;
  max: number;
  /** 0 for sensors that only offer a fixed list of values. */
  step: number;
}

export interface ReportRateState {
  currentHz: number;
  availableHz: number[];
  extended: boolean;
}

export interface Capabilities {
  dpi: boolean;
  reportRate: boolean;
  battery: boolean;
  lighting: boolean;
  onboardMemory: boolean;
}

export interface Device {
  id: string;
  name: string;
  kind: DeviceKind;
  vendorId: number;
  productId: number;
  serial: string | null;
  /** The device's own product ids; behind a receiver `productId` is the dongle's. */
  modelIds: number[];
  connection: Connection;
  online: boolean;
  capabilities: Capabilities;
  battery: BatteryState | null;
  dpi: DpiState | null;
  reportRate: ReportRateState | null;
  lightingZones: number;
  protocolVersion: string;
  demo: boolean;
  lastError: string | null;
}

/** Mirrors `state::EmptyReason` — serde externally-tagged enum. */
export type EmptyReason =
  | "noHardware"
  | "noResponse"
  | { permissionDenied: { devices: string[] } }
  | { hidUnavailable: { message: string } };

export interface DeviceListPayload {
  devices: Device[];
  demo: boolean;
  error: string | null;
  emptyReason: EmptyReason | null;
}

export interface BatteryEvent {
  deviceId: string;
  battery: BatteryState;
}

export interface FeatureInfo {
  id: string;
  name: string;
  index: number;
  obsolete: boolean;
  hidden: boolean;
}

export type LightEffectName = "off" | "fixed" | "breathing" | "cycle";

/** One addressable lighting zone, from HID++ `getZoneInfo`. */
export interface ZoneInfo {
  index: number;
  location: number;
  /** "Primary", "Logo", … — G HUB uses these as its zone tab labels. */
  locationName: string;
  /** Effect ids the zone accepts: 0x00 off, 0x01 fixed, 0x03 cycle, 0x0a breathing. */
  effects: number[];
}

export interface LightingSettings {
  effect: LightEffectName;
  color: string;
  brightness: number;
  rateMs: number;
}

export interface Assignment {
  control: string;
  category: "command" | "key" | "action" | "macro" | "system";
  label: string;
  value: string;
}

export interface DeviceProfile {
  dpiStages: number[];
  activeStage: number;
  reportRateHz: number | null;
  lighting: LightingSettings | null;
  /** Per-zone settings, keyed by zone index. */
  lightingZones: Record<string, LightingSettings>;
  assignments: Assignment[];
}

export interface Profile {
  id: string;
  name: string;
  kind: "desktop" | "game" | "app";
  devices: Record<string, DeviceProfile>;
}

export interface Settings {
  startMinimised: boolean;
  showBatteryNotifications: boolean;
  batteryPollSeconds: number;
  illuminationFollowsProfile: boolean;
}

export interface Config {
  profiles: Profile[];
  activeProfile: string;
  settings: Settings;
}

export interface LightingRequest {
  deviceId: string;
  /** Zone index, or 0xff for every zone at once. */
  zone: number;
  color: string;
  effect: LightEffectName;
  brightness: number;
  rateMs: number;
  /** Also write the device's flash, so the effect survives a reconnect. */
  persist?: boolean;
}
