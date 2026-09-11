/** Profiles and application settings, persisted by the backend. */
import * as api from "$lib/api";
import type { Config, DeviceProfile, Profile, Settings } from "$lib/types";

const EMPTY_DEVICE_PROFILE: DeviceProfile = {
  dpiStages: [],
  activeStage: 0,
  reportRateHz: null,
  lighting: null,
  lightingZones: {},
  assignments: [],
};

class ConfigStore {
  profiles = $state<Profile[]>([]);
  activeProfileId = $state("default");
  settings = $state<Settings>({
    startMinimised: false,
    showBatteryNotifications: true,
    batteryPollSeconds: 60,
    illuminationFollowsProfile: false,
  });
  loading = $state(true);

  readonly active = $derived(
    this.profiles.find((p) => p.id === this.activeProfileId) ?? this.profiles[0],
  );

  async load() {
    this.loading = true;
    try {
      this.apply(await api.getConfig());
    } finally {
      this.loading = false;
    }
  }

  apply(config: Config) {
    this.profiles = config.profiles;
    this.activeProfileId = config.activeProfile;
    this.settings = config.settings;
  }

  deviceProfile(deviceId: string): DeviceProfile {
    const stored = this.active?.devices[deviceId];
    if (!stored) return structuredClone(EMPTY_DEVICE_PROFILE);
    // Older configs predate per-zone lighting.
    return { ...structuredClone(EMPTY_DEVICE_PROFILE), ...stored };
  }

  async saveDeviceProfile(deviceId: string, profile: DeviceProfile) {
    this.apply(await api.saveDeviceProfile(deviceId, profile));
  }

  async selectProfile(id: string) {
    this.apply(await api.setActiveProfile(id));
  }

  async createProfile(name: string, kind = "game") {
    this.apply(await api.createProfile(name, kind));
  }

  async removeProfile(id: string) {
    this.apply(await api.deleteProfile(id));
  }

  async saveSettings(settings: Settings) {
    this.apply(await api.saveSettings(settings));
  }
}

export const configStore = new ConfigStore();
