// Mirrors the engine's JSON (snake_case, as serde writes it).

export type SupportStatus =
  | 'implemented_tested'
  | 'implemented_not_hardware_verified'
  | 'partially_implemented'
  | 'simulated_only'
  | 'planned'
  | 'unsupported';

export type Tri = 'yes' | 'no' | 'unknown';

export interface Device {
  id: string;
  kind: string;
  name: string;
  detail: string;
  address: string;
  simulated: boolean;
}

export interface LinkInfo {
  kind: string;
  name: string;
  simulated: boolean;
}

export interface Adapter {
  family: string;
  link: string;
  identity: string;
  description: string | null;
  stn_firmware: string | null;
  protocols: string[];
  vehicle_protocol: string | null;
  headers: Tri;
  spaces_off: Tri;
  voltage_reading: Tri;
  multi_frame: Tri;
  can_29bit: Tri;
  max_can_frame_bytes: number | null;
  clone_suspected: boolean;
  notes: string[];
}

export interface FunctionStatus {
  id: string;
  name: string;
  risk_level: number;
  status: SupportStatus;
  status_label: string;
  note: string;
  local_result: 'works' | 'partial' | 'fails' | 'not_supported' | null;
  best_verification: string | null;
}

export interface CompatStatus {
  simulated: boolean;
  platform: string;
  adapter: string;
  adapter_known_records: number;
  vehicle_known_records: number;
  summary: string;
  functions: FunctionStatus[];
}

export interface Summary {
  simulated: boolean;
  link: LinkInfo;
  adapter: Adapter;
  protocol: string | null;
  vehicle_connected: boolean;
  vehicle_error: string | null;
  ecus: { address: string; label: string; supported_pids: number }[];
  adapter_voltage: number | null;
  session_id: number;
  compatibility: CompatStatus;
}

export type Severity = 'info' | 'low' | 'moderate' | 'high' | 'critical';

export interface Explanation {
  code: string;
  system: string;
  ownership: 'generic' | 'manufacturer' | 'reserved' | 'unknown';
  known: boolean;
  title: string;
  plain: string;
  technical: string;
  severity: Severity;
  severity_note: string;
  possible_causes: string[];
  suggested_tests: string[];
  related_measurements: string[];
  driving_advice: string | null;
  source: string;
}

export type DtcKind = 'stored' | 'pending' | 'permanent';

export interface DtcFinding {
  code: string;
  ecu: string;
  ecu_label: string;
  kind: DtcKind;
  explanation: Explanation;
}

export interface Measurement {
  key: string;
  name: string;
  value: number;
  unit: string;
  ecu: string;
  source: string;
}

export interface Monitor {
  name: string;
  available: boolean;
  complete: boolean;
}

export interface MonitorStatus {
  mil_on: boolean;
  dtc_count: number;
  compression_ignition: boolean;
  monitors: Monitor[];
}

export interface VinInfo {
  vin: string;
  valid_format: boolean;
  check_digit_ok: boolean | null;
  wmi: string;
  manufacturer: string | null;
  model_years: number[];
  warnings: string[];
}

export interface Report {
  id: string;
  created_at: string;
  app: string;
  app_version: string;
  platform: string;
  simulated: boolean;
  link: LinkInfo;
  adapter: Adapter;
  protocol: string | null;
  vehicle: {
    vin: VinInfo | null;
    calibration_ids: [string, string][];
    ecu_names: [string, string][];
    obd_standard: string | null;
    fuel_type: string | null;
    note: string;
  };
  modules: { category: string; ecu: string | null; label: string; status: 'scanned' | 'partly_scanned' | 'not_scanned'; detail: string }[];
  dtcs: DtcFinding[];
  readiness: [string, MonitorStatus][];
  freeze_frames: { ecu: string; dtc: string | null; values: Measurement[] }[];
  measurements: Measurement[];
  warnings: string[];
  communication_errors: string[];
  health: { verdict: 'faults_found' | 'no_faults_in_scanned_modules' | 'incomplete'; headline: string; detail: string };
  notes: string;
  operations: { at: string; what: string; detail: string }[];
  cautions: string[];
}

export interface ReportSummary {
  id: number;
  title: string;
  created_at: string;
  vehicle_id: number | null;
  simulated: boolean;
  dtc_count: number;
}

export interface LiveParam {
  key: string;
  name: string;
  unit: string;
  min: number;
  max: number;
  category: string;
  common: boolean;
  source: string;
}

export interface LiveSample {
  key: string;
  name: string;
  unit: string;
  value: number | null;
  ecu: string | null;
  source: string;
  error: string | null;
  at_ms: number;
}

export interface Decision {
  allowed: boolean;
  blockers: string[];
  warnings: string[];
}

export interface ClearPlan {
  id: string;
  simulated: boolean;
  expires_in_seconds: number;
  codes: DtcFinding[];
  permanent: DtcFinding[];
  affected_modules: [string, string][];
  freeze_frames: Report['freeze_frames'];
  consequences: string[];
  preview: Decision;
  snapshot_id: number;
}

export interface ClearResult {
  performed: boolean;
  decision: Decision;
  modules: { ecu: string; label: string; accepted: boolean; message: string }[];
  remaining: DtcFinding[];
  summary: string;
}

export interface Settings {
  mode: 'beginner' | 'expert';
  units: 'metric' | 'imperial';
  theme: 'system' | 'light' | 'dark';
  language: string;
  terms_accepted: boolean;
  terms_version: string;
  configuration_enabled: boolean;
  show_vin_in_log: boolean;
  platform: string;
}

export interface Vehicle {
  id: number;
  label: string;
  vin: string | null;
  make: string | null;
  model: string | null;
  year: number | null;
  kind: string | null;
  notes: string;
  simulated: boolean;
  created_at: string;
}

export interface Step {
  id: string;
  kind: 'question' | 'info' | 'measurement' | 'outcome';
  text: string | null;
  help: string | null;
  measurement: { unit: string; user_spec?: boolean } | null;
  live_pid: string | null;
}

export interface Procedure {
  id: string;
  title: string;
  summary: string;
  applies_to: { vehicle_kinds: string[]; codes: string[]; systems: string[] };
  source: string;
  verification: string;
  steps: Step[];
}

export interface Answer {
  step: string;
  choice?: string;
  value?: number;
  value_source?: 'measured_by_user' | 'read_from_vehicle';
  spec_min?: number;
  spec_max?: number;
}

export interface Evaluated {
  step: string;
  text: string;
  choice: string | null;
  measured: number | null;
  value_source: string | null;
  unit: string | null;
  spec: { min: number; max: number; kind: 'documented' | 'general_guidance' | 'derived'; source: string } | null;
  comparison: 'in_range' | 'below' | 'above' | null;
  explanation: string | null;
}

export interface GuidedState {
  procedure_id: string;
  current: Step | null;
  history: Evaluated[];
  outcome: { kind: 'finding' | 'hypothesis'; text: string } | null;
  error: string | null;
}

export interface CompatRecord {
  uuid?: string;
  make: string;
  model: string;
  year: number | null;
  ecu_hw: string;
  ecu_sw: string;
  adapter: string;
  adapter_firmware: string;
  os: string;
  protocol: string;
  function: string;
  result: 'works' | 'partial' | 'fails' | 'not_supported';
  verification: string;
  app_version: string;
  evidence: string[];
  notes: string;
  source?: string;
  version?: number;
  updated_at?: string;
}

export interface Submission {
  id: number;
  record: CompatRecord;
  status: string;
  moderator_note: string;
  created_at: string;
}

export interface SimSettings {
  engine_running: boolean;
  latency_ms: number;
  faults: {
    ignition_off: boolean;
    no_response: boolean;
    disconnect_after: number | null;
    malformed: boolean;
    truncated: boolean;
    low_voltage: boolean;
    wrong_vin: boolean;
    bus_error: boolean;
    clone_adapter: boolean;
  };
}

export interface CatalogEntry {
  id: string;
  name: string;
  risk_level: number;
  status: SupportStatus;
  status_label: string;
  windows: SupportStatus;
  android: SupportStatus;
  linux: SupportStatus;
  note: string;
}

export type ConnectorCategory = 'car' | 'motorcycle' | 'truck' | 'agricultural' | 'powersports' | 'marine' | 'light_electric';

export interface Connector {
  id: string;
  name: string;
  category: ConnectorCategory;
  makes: string[];
  applies_to: string;
  pins: number | null;
  description: string;
  voltage: string;
  access: 'data_link' | 'blink_codes' | 'data_link_or_blink_codes' | 'dashboard_self_test' | 'no_diagnostics';
  protocols: string[];
  pinout: { pin: string; signal: string }[];
  elm327: 'yes' | 'with_cable' | 'limited' | 'no' | 'not_needed';
  adapter: string;
  support: SupportStatus;
  support_note: string;
  how_to_read: string[];
  warnings: string[];
  confidence: 'low' | 'medium' | 'high';
  sources: { url: string; note: string }[];
  notes: string[];
}
