/** Shared presentation helpers for devices — icons, labels, default tabs. */
import type { IconName } from "$lib/components/Icon.svelte";
import type { BatteryState, Connection, Device, DeviceKind } from "$lib/types";

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

export type TabId = "sensitivity" | "assignments" | "lighting" | "settings";

/** The tabs a device can actually drive, in G HUB's order. */
export function tabsFor(device: Device): { id: TabId; label: string; icon: IconName }[] {
  const tabs: { id: TabId; label: string; icon: IconName }[] = [];
  if (device.capabilities.lighting) {
    tabs.push({ id: "lighting", label: "LIGHTSYNC", icon: "lightsync" });
  }
  if (device.kind === "mouse" || device.kind === "keyboard") {
    tabs.push({ id: "assignments", label: "Assignments", icon: "assignments" });
  }
  if (device.capabilities.dpi) {
    tabs.push({ id: "sensitivity", label: "Sensitivity", icon: "dpi" });
  }
  tabs.push({ id: "settings", label: "Settings", icon: "sliders" });
  return tabs;
}

export function defaultTab(device: Device): TabId {
  return tabsFor(device)[0].id;
}
