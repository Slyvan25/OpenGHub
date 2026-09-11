/**
 * Live device list.
 *
 * The backend is the source of truth; this class caches the last snapshot so
 * navigation is instant, and patches it in place when `battery-update` /
 * `device-updated` events arrive rather than refetching the whole list.
 */
import * as api from "$lib/api";
import type { BatteryEvent, Device, DeviceListPayload, EmptyReason } from "$lib/types";

class DeviceStore {
  devices = $state<Device[]>([]);
  demo = $state(false);
  /** Set when hidapi itself failed to start (usually a permissions problem). */
  error = $state<string | null>(null);
  /** Why the list is empty, when it is — see the banner on the dashboard. */
  emptyReason = $state<EmptyReason | null>(null);
  loading = $state(true);
  lastRefresh = $state<number>(0);

  #subscribed = false;

  async load(refresh = true) {
    this.loading = true;
    try {
      this.apply(await api.getConnectedDevices(refresh));
    } catch (e) {
      this.error = api.errorMessage(e);
    } finally {
      this.loading = false;
    }
  }

  apply(payload: DeviceListPayload) {
    this.devices = payload.devices;
    this.demo = payload.demo;
    this.error = payload.error;
    this.emptyReason = payload.emptyReason ?? null;
    this.lastRefresh = Date.now();
  }

  /** Attaches the backend event listeners. Safe to call more than once. */
  async subscribe(): Promise<() => void> {
    if (this.#subscribed) return () => {};
    this.#subscribed = true;

    const unsubs = await Promise.all([
      api.on<DeviceListPayload>(api.events.devicesChanged, (p) => this.apply(p)),
      api.on<BatteryEvent>(api.events.batteryUpdate, (e) => this.patchBattery(e)),
      api.on<Device>(api.events.deviceUpdated, (d) => this.patch(d)),
    ]);

    return () => {
      unsubs.forEach((fn) => fn());
      this.#subscribed = false;
    };
  }

  get(id: string): Device | undefined {
    return this.devices.find((d) => d.id === id);
  }

  patch(device: Device) {
    const i = this.devices.findIndex((d) => d.id === device.id);
    if (i >= 0) this.devices[i] = device;
  }

  patchBattery({ deviceId, battery }: BatteryEvent) {
    const device = this.get(deviceId);
    if (device) device.battery = battery;
  }

  async setDpi(id: string, dpi: number) {
    const state = await api.setDeviceDpi(id, dpi);
    const device = this.get(id);
    if (device) device.dpi = state;
    return state;
  }

  async setReportRate(id: string, hz: number) {
    const state = await api.setPollingRate(id, hz);
    const device = this.get(id);
    if (device) device.reportRate = state;
    return state;
  }

  /** Refreshes one device's live values (battery, DPI, rate). */
  async refreshDevice(id: string) {
    const device = await api.getDeviceState(id);
    this.patch(device);
    return device;
  }
}

export const deviceStore = new DeviceStore();
