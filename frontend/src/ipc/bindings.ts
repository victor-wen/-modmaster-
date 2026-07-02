export type { UnlistenFn } from "@tauri-apps/api/event";

export interface Project { name: string; version: number; runtime: ProjectRuntime; storage: ProjectStorage; }
export interface ProjectRuntime { default_poll_interval_ms: number; }
export interface ProjectStorage { history_sampling_ms: number; trend_max_points: number; }
export interface Device {
  id: string; name: string; enabled: boolean; protocol: string;
  transport: TcpTransport | RtuTransport;
  protocol_params: Record<string, unknown>;
  poll_interval_ms: number; timeout_ms: number;
}
export interface TcpTransport { type: "Tcp"; host: string; port: number; }
export interface RtuTransport { type: "Rtu"; port: string; baud: number; data_bits: number; parity: string; stop_bits: number; }
export interface Tag {
  id: string; device_id: string; name: string; enabled: boolean;
  data_type: string; byte_order: string; scale: number; offset: number;
  unit: string; writable: boolean; protocol_params: Record<string, unknown>;
}
export interface TagUpdate { tag_id: string; ts: string; value: number | boolean; unit: string; quality: string; }
export interface DeviceState { device_id: string; online: boolean; error_count: number; last_error: string | null; last_poll_at: string | null; }
export interface RuntimeStatus { running: boolean; devices: DeviceState[]; }
export interface Sample { tag_id: string; ts: string; value: number | boolean; quality: string; }
export interface LogEntry { ts: string; level: string; message: string; }
