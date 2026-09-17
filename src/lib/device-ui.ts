/** Shared presentation helpers for devices — icons, labels, default tabs. */
import type { IconName } from "$lib/components/Icon.svelte";
import type { BatteryState, Connection, Device, DeviceKind, DeviceProfile } from "$lib/types";

export function kindIcon(kind: DeviceKind): IconName {
  switch (kind) {
    case "mouse":
    case "keyboard":
    case "headset":
    case "microphone":
    case "light":
    case "speaker":
    case "webcam":
    case "wheel":
    case "receiver":
      return kind;
    default:
      return "device";
  }
}

export function connectionIcon(connection: Connection): IconName {
  switch (connection) {
    case "wired":
      return "usb";
    case "bluetooth":
      return "bluetooth";
    default:
      return "wireless";
  }
}

export function connectionLabel(connection: Connection): string {
  switch (connection) {
    case "wired":
      return "USB";
    case "bluetooth":
      return "Bluetooth";
    case "receiver":
      return "LIGHTSPEED receiver";
    default:
      return "Wireless";
  }
}

export function batteryIcon(battery: BatteryState): IconName {
  return battery.status === "discharging" || battery.status === "full"
    ? "battery"
    : "batteryCharging";
}

export function batteryLabel(battery: BatteryState): string {
  switch (battery.status) {
    case "charging":
    case "slowCharging":
      return "Charging";
    case "chargingFull":
      return "Almost full";
    case "full":
      return "Fully charged";
    case "error":
      return "Battery fault";
    case "unknown":
      return "Unknown";
    default:
      return battery.approximate ? "Estimated" : "On battery";
  }
}

/**
 * Product ids to try when looking for artwork, best first. Behind a receiver the
 * `productId` is the dongle's, so the device's own model ids matter more.
 */
export function artworkIds(device: Pick<Device, "productId" | "modelIds">): number[] {
  return [...(device.modelIds ?? []), device.productId].filter(
    (id, i, all) => id !== 0 && all.indexOf(id) === i,
  );
}

/**
 * Per-zone glows for a device from its saved profile — what the dashboard
 * needs to show the same lighting the LIGHTSYNC page previews. Falls back to
 * the single legacy colour when no per-zone settings exist yet.
 */
export function zoneGlowsFor(
  profile: Pick<DeviceProfile, "lightingZones" | "zoneNames" | "lighting">,
): { index: number; locationName: string; color: string | null; brightness: number }[] {
  const names = profile.zoneNames ?? [];
  const zones = profile.lightingZones ?? {};
  if (names.length === 0) return [];
  return names.map((locationName, index) => {
    const s = zones[String(index)] ?? profile.lighting ?? null;
    return {
      index,
      locationName,
      color: s && s.effect !== "off" ? s.color : null,
      brightness: s?.brightness ?? 0,
    };
  });
}

export type TabId = "sensitivity" | "assignments" | "lighting" | "wheel" | "pedals" | "settings";

/**
 * The tabs a device can actually drive, in G HUB's rail order for a mouse:
 * Sensitivity, Assignments, LIGHTSYNC, with the gear apart at the bottom.
 */
export function tabsFor(device: Device): { id: TabId; label: string; icon: IconName }[] {
  const tabs: { id: TabId; label: string; icon: IconName }[] = [];
  if (device.capabilities.wheel) {
    // G HUB's wheel rail: assignments, then the wheel page.
    tabs.push({ id: "assignments", label: "Assignments", icon: "assignments" });
    tabs.push({ id: "wheel", label: "Steering Wheel", icon: "wheel" });
    tabs.push({ id: "pedals", label: "Pedals", icon: "pedals" });
    tabs.push({ id: "settings", label: "Settings", icon: "gear" });
    return tabs;
  }
  if (device.capabilities.dpi) {
    tabs.push({ id: "sensitivity", label: "Sensitivity", icon: "dpi" });
  }
  if (device.kind === "mouse" || device.kind === "keyboard") {
    tabs.push({ id: "assignments", label: "Assignments", icon: "assignments" });
  }
  if (device.capabilities.lighting) {
    tabs.push({ id: "lighting", label: "LIGHTSYNC", icon: "lightsync" });
  }
  tabs.push({ id: "settings", label: "Settings", icon: "gear" });
  return tabs;
}

export function defaultTab(device: Device): TabId {
  if (device.capabilities.wheel) return "wheel";
  return tabsFor(device)[0].id;
}
