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
  /** A racing wheel driven through the classic command channel. */
  wheel?: boolean;
}

export interface FirmwareInfo {
  kind: string;
  version: string;
  active: boolean;
}

/** G HUB's per-device settings, outside the profiles. */
export interface DeviceSettings {
  autoSleepMin: number;
  inactivityLightingMin: number;
  lowBatteryMode: boolean;
  lowBatteryThreshold: number;
  lowBatteryBrightness: number;
  leftHanded: boolean;
}

/** Static facts about a wheel. */
export interface WheelInfo {
  rangeMin: number;
  rangeMax: number;
  rpmLeds: number;
  protocol: string;
  driverRunning: boolean;
  hardwareCalibration: boolean;
}

/** Live wheel input: steering −1..1, pedals 0..1. */
export interface WheelState {
  steeringRaw: number;
  steering: number;
  accelerator: number;
  brake: number;
  clutch: number;
  buttons: number;
  hat: number;
}

/** G HUB's Steering Wheel panel, per profile. */
export interface WheelSettings {
  rangeDeg: number;
  sensitivity: number;
  centerSpring: number;
  centerSpringInFfbGames: boolean;
  ffbGain: number;
  centerOffset: number;
  trueforceTorque: number;
  trueforceAudio: number;
  trueforceGameControl: boolean;
  pedals: PedalSettings;
}

export interface PedalCurve {
  sensitivity: number;
  deadZoneLow: number;
  deadZoneHigh: number;
  inverted: boolean;
}

export interface PedalSettings {
  accelerator: PedalCurve;
  brake: PedalCurve;
  clutch: PedalCurve;
  combined: boolean;
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
  /** true = running its onboard profile; null for devices without one. */
  onboardMode?: boolean | null;
  firmware?: FirmwareInfo[];
  wheel?: WheelInfo | null;
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

export type LightEffectName = "off" | "fixed" | "breathing" | "cycle" | "screen" | "audio";

/** A screen region as fractions of the monitor. */
export interface Region {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Parameters of a software effect run by the app (screen sampler / audio visualizer). */
export type SoftwareEffect =
  | { kind: "screen"; region: Region; brightness: number }
  | { kind: "audio"; low: string; mid: string; high: string; sensitivity: number; brightness: number };

export interface LightSyncStatus {
  activeZones: number;
  error: string | null;
  screenAuthorised: boolean;
}

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
  software?: SoftwareEffect | null;
}

export interface Assignment {
  control: string;
  category: "command" | "key" | "action" | "macro" | "system";
  label: string;
  value: string;
}

export type MacroKind = "noRepeat" | "repeatWhileHolding" | "toggle" | "sequence";

export interface MacroSections {
  onPress: import("$lib/macros").MacroStep[];
  whileHolding: import("$lib/macros").MacroStep[];
  onRelease: import("$lib/macros").MacroStep[];
}

export interface MacroDef {
  id: string;
  name: string;
  /** What the device plays: the flattened sequence. */
  steps: import("$lib/macros").MacroStep[];
  /** G HUB's macro type; presentation only, the device always gets `steps`. */
  kind?: MacroKind | null;
  sections?: MacroSections | null;
  useStandardDelays?: boolean | null;
  standardDelayMs?: number | null;
  color?: string | null;
}

export interface DeviceProfile {
  dpiStages: number[];
  activeStage: number;
  /** Stage a DPI-shift button jumps to while held; null when none is marked. */
  shiftStage?: number | null;
  reportRateHz: number | null;
  lighting: LightingSettings | null;
  /** Per-zone settings, keyed by zone index. */
  lightingZones: Record<string, LightingSettings>;
  /** Dragged glow positions, keyed by zone index. */
  zonePositions: Record<string, { x: number; y: number; r: number }>;
  /** HID++ location name per zone index, cached from the device. */
  zoneNames: string[];
  assignments: Assignment[];
  /** Recorded macros, referenced by assignments with category `macro`. */
  macros: MacroDef[];
  /** Steering wheel settings, for wheels. */
  wheel?: WheelSettings | null;
}

/** An entry from Logitech's public application database. */
export interface Application {
  id: string;
  name: string;
  posterUrl: string | null;
  steamAppIds: string[];
  executables: string[];
  commandCount: number;
}

export interface ApplicationCommand {
  category: string;
  name: string;
  keystroke: string[];
}

export interface ApplicationCommands {
  id: string;
  name: string;
  commands: ApplicationCommand[];
  categoryColors: { hex: string; tag: string }[];
}

export interface DatabaseInfo {
  version: string;
  applicationCount: number;
  fetchedAt: number;
  cachePath: string;
}

export interface Profile {
  id: string;
  name: string;
  kind: "desktop" | "game" | "app";
  /** Bound game from the application database; activates when it runs. */
  applicationId?: string | null;
  posterUrl?: string | null;
  /** A disabled game never activates its profile. */
  disabled?: boolean;
  devices: Record<string, DeviceProfile>;
}

export interface Settings {
  startMinimised: boolean;
  showBatteryNotifications: boolean;
  batteryPollSeconds: number;
  illuminationFollowsProfile: boolean;
  autoSwitchProfiles: boolean;
  autoFetchArtwork: boolean;
  /** Profile used when no bound game is running. */
  persistentProfile: string;
  /** Base URL of the community profile repository. */
  communityRepo: string;
  /** Name written into shared profiles. */
  authorName: string;
  /** Run the userspace force-feedback driver for classic wheels. */
  wheelDriver?: boolean;
  screenRestoreToken?: string | null;
  /** Devices switched to on-board memory mode. */
  onboardModeDevices?: string[];
}

// -- community profiles ----------------------------------------------------

export interface TargetDevice {
  modelId: string;
  productIds: number[];
  displayName: string;
  kind: string;
}

export interface SharedProfile {
  format: number;
  id: string;
  name: string;
  author: string;
  description: string;
  license: string;
  device: TargetDevice;
  application: { id: string; name: string } | null;
  profile: DeviceProfile;
}

export interface CommunityEntry {
  id: string;
  name: string;
  author: string;
  description: string;
  device: TargetDevice;
  application: { id: string; name: string } | null;
  path: string;
  macroCount: number;
  assignmentCount: number;
  dpiStages: number[];
  hasLighting: boolean;
  updated: string;
}

export interface CommunityIndex {
  version: number;
  generated: string;
  profiles: CommunityEntry[];
}

export interface Config {
  profiles: Profile[];
  activeProfile: string;
  settings: Settings;
  manualGames?: ManualGame[];
}

// -- games library ----------------------------------------------------------

export type GameSource = "steam" | "epic" | "gog" | "lutris" | "manual";

/** One installed game, gathered from a launcher on this machine. */
export interface Game {
  /** `<source>:<launcher key>`. */
  id: string;
  source: GameSource;
  name: string;
  /** Local cover file (served through the asset protocol). */
  cover: string | null;
  /** Remote cover to fall back on. */
  coverUrl: string | null;
  /** Unix seconds; 0 when never played. */
  lastPlayed: number;
  playtimeMinutes: number;
  installDir: string | null;
  /** Matching Logitech application, which is what links a game to a profile. */
  applicationId: string | null;
}

/** An executable the user added to the library by hand. */
export interface ManualGame {
  id: string;
  name: string;
  exec: string;
  args: string;
  cover: string | null;
}

/** Zone rectangles and button markers imported from a G HUB depot, normalised 0–1. */
export interface ArtworkLayout {
  modelId: string;
  displayName: string;
  views: ArtworkView[];
}

export interface ArtworkView {
  view: "front" | "side" | string;
  width: number;
  height: number;
  zones: { id: string; locationName: string; x: number; y: number; width: number; height: number }[];
  controls: {
    slotId: string;
    control: string;
    markerX: number;
    markerY: number;
    labelX: number;
    labelY: number;
    side: "left" | "right" | "top";
  }[];
}

export interface GhubCacheInfo {
  buildId: string;
  version: string;
  depots: number;
  deviceDefinitions: number;
}

export interface ImportedDevice {
  modelId: string;
  displayName: string;
  productIds: number[];
  views: string[];
  hasThumbnail: boolean;
}

export interface ImportReport {
  buildId: string;
  depotsInDepository: number;
  deviceDefinitions: number;
  imported: ImportedDevice[];
  artworkDir: string;
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
