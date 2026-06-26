import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, FolderGit2, Loader2, Network, Plus, RefreshCw } from "lucide-react";
import { Button } from "../components/ui/button";
import { Badge } from "../components/ui/badge";
import { Card, CardContent } from "../components/ui/card";
import type { PortProjectMap, ProjectSummary, ServiceInfo } from "../types";

interface DashboardProps {
  onSelectProject: (path: string) => void;
}

type Status = "conflict" | "running" | "idle";

const CHIP: Record<Status, string> = {
  conflict: "bg-destructive text-destructive-foreground",
  running: "bg-primary text-primary-foreground",
  idle: "bg-muted text-muted-foreground",
};

const DOT: Record<Status, string> = {
  conflict: "bg-destructive",
  running: "bg-success",
  idle: "bg-muted-foreground/40",
};

function StatCard({
  icon,
  label,
  value,
  tone = "default",
}: {
  icon: React.ReactNode;
  label: string;
  value: number;
  tone?: "default" | "danger";
}) {
  return (
    <Card>
      <CardContent className="flex items-center gap-4 p-5">
        <span
          className={`flex h-12 w-12 items-center justify-center rounded-xl ${
            tone === "danger" ? "bg-destructive/10 text-destructive" : "bg-primary/10 text-primary"
          }`}
        >
          {icon}
        </span>
        <div>
          <div
            className={`tnum text-3xl font-bold leading-none ${
              tone === "danger" && value > 0 ? "text-destructive" : ""
            }`}
          >
            {value}
          </div>
          <div className="mt-1.5 text-sm text-muted-foreground">{label}</div>
        </div>
      </CardContent>
    </Card>
  );
}

export default function Dashboard({ onSelectProject }: DashboardProps) {
  const [ports, setPorts] = useState<PortProjectMap[]>([]);
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = () => {
    setLoading(true);
    setError(null);
    Promise.all([
      invoke<ProjectSummary[]>("get_projects"),
      invoke<PortProjectMap[]>("get_port_project_map"),
    ])
      .then(([proj, prts]) => {
        setProjects(proj);
        setPorts(prts);
      })
      .catch((e: unknown) => setError(String(e)))
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    load();
  }, []);

  if (error) {
    return (
      <Card className="bg-destructive/5">
        <CardContent className="flex items-start gap-3 p-6">
          <AlertTriangle className="mt-0.5 h-5 w-5 shrink-0 text-destructive" />
          <div className="space-y-2">
            <p className="font-medium text-destructive">Couldn't load data</p>
            <p className="text-sm text-muted-foreground">{error}</p>
            <Button size="sm" variant="outline" onClick={load}>
              Try again
            </Button>
          </div>
        </CardContent>
      </Card>
    );
  }

  if (loading && projects.length === 0) {
    return (
      <div className="flex h-72 flex-col items-center justify-center gap-3 text-muted-foreground">
        <Loader2 className="h-6 w-6 animate-spin" />
        <p className="text-sm">Loading projects…</p>
      </div>
    );
  }

  // Live signals derived from the system scan.
  const listening = new Set(ports.map((p) => p.port));
  const conflicted = new Set(ports.filter((p) => p.is_conflict).map((p) => p.port));
  const conflictCount = conflicted.size;

  const statusOf = (s: ServiceInfo): Status => {
    if (conflicted.has(s.assigned)) return "conflict";
    if (s.pid != null || listening.has(s.assigned)) return "running";
    return "idle";
  };

  return (
    <div className="animate-fade-in space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold tracking-tight">Projects</h1>
        <Button variant="outline" size="sm" className="gap-1.5" onClick={load} disabled={loading}>
          <RefreshCw className={`h-4 w-4 ${loading ? "animate-spin" : ""}`} />
          Rescan
        </Button>
      </div>

      <div className="grid grid-cols-3 gap-5">
        <StatCard icon={<FolderGit2 className="h-6 w-6" />} label="Tracked projects" value={projects.length} />
        <StatCard icon={<Network className="h-6 w-6" />} label="Listening ports" value={ports.length} />
        <StatCard
          icon={<AlertTriangle className="h-6 w-6" />}
          label="Conflicts"
          value={conflictCount}
          tone="danger"
        />
      </div>

      {projects.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center gap-3 py-14 text-center">
            <FolderGit2 className="h-8 w-8 text-muted-foreground" />
            <div>
              <p className="font-medium">No projects tracked yet</p>
              <p className="text-sm text-muted-foreground">
                Start one with{" "}
                <code className="rounded bg-muted px-1.5 py-0.5">portier run -- npm run dev</code>{" "}
                and it shows up here automatically.
              </p>
            </div>
          </CardContent>
        </Card>
      ) : (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(300px,1fr))] gap-5">
          {projects.map((proj) => {
            const projStatus: Status = proj.services.some((s) => statusOf(s) === "conflict")
              ? "conflict"
              : proj.services.some((s) => statusOf(s) === "running")
                ? "running"
                : "idle";
            return (
              <Card key={proj.path}>
                <CardContent className="space-y-4 p-5">
                  <div className="flex items-start justify-between gap-2">
                    <button
                      onClick={() => onSelectProject(proj.path)}
                      className="flex items-center gap-2 text-left"
                    >
                      <span className={`h-2.5 w-2.5 shrink-0 rounded-full ${DOT[projStatus]}`} />
                      <span className="font-semibold hover:text-primary hover:underline">
                        {proj.name}
                      </span>
                    </button>
                    <Badge variant="secondary">{proj.stack}</Badge>
                  </div>

                  {proj.services.length === 0 ? (
                    <p className="text-xs text-muted-foreground">No ports assigned.</p>
                  ) : (
                    <div className="flex flex-wrap gap-1.5">
                      {proj.services.map((s) => (
                        <span
                          key={s.name}
                          title={`${s.name}: preferred ${s.preferred}, assigned ${s.assigned}`}
                          className={`tnum rounded-md px-2 py-1 text-xs font-medium ${CHIP[statusOf(s)]}`}
                        >
                          {s.name} · {s.assigned}
                        </span>
                      ))}
                    </div>
                  )}
                </CardContent>
              </Card>
            );
          })}
        </div>
      )}

      <div className="flex flex-wrap items-center gap-x-4 gap-y-1.5 text-xs text-muted-foreground">
        <span className="flex items-center gap-1.5">
          <span className={`h-2.5 w-2.5 rounded-full ${DOT.running}`} /> Running
        </span>
        <span className="flex items-center gap-1.5">
          <span className={`h-2.5 w-2.5 rounded-full ${DOT.conflict}`} /> Conflict
        </span>
        <span className="flex items-center gap-1.5">
          <span className={`h-2.5 w-2.5 rounded-full ${DOT.idle}`} /> Idle
        </span>
        <span className="ml-auto">
          <Plus className="mr-1 inline h-3 w-3" />
          Projects are tracked automatically when started with{" "}
          <code className="rounded bg-muted px-1 py-0.5">portier run</code>
        </span>
      </div>
    </div>
  );
}
