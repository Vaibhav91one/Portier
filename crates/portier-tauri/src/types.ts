export interface PortStatus { port: number; pids: number[]; process_names: string[]; is_conflict: boolean; }
export interface PortProjectMap {
  port: number; pids: number[]; process_names: string[];
  is_conflict: boolean; project_name: string | null; service_name: string | null;
}
export interface ServiceEntry { preferred: number; assigned: number; pid: number | null; }
export interface ProjectEntry {
  name: string; stack: string; services: Record<string, ServiceEntry>;
  linked: boolean; worktree: string | null;
}
export interface ProjectSummary { name: string; stack: string; path: string; services_count: number; }