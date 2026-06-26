import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, FolderGit2, Loader2, Network, RefreshCw } from "lucide-react";
import { Button } from "../components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import PortMap, { PortMapLegend } from "../components/PortMap";
import type { PortProjectMap, ProjectSummary } from "../types";

interface DashboardProps {
  onSelectProject: (path: string) => void;
}

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
      <Card className="border-destructive/50">
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

  if (loading && ports.length === 0 && projects.length === 0) {
    return (
      <div className="flex h-72 flex-col items-center justify-center gap-3 text-muted-foreground">
        <Loader2 className="h-6 w-6 animate-spin" />
        <p className="text-sm">Scanning ports…</p>
      </div>
    );
  }

  const conflictCount = ports.filter((p) => p.is_conflict).length;
  const pathByName = new Map(projects.map((p) => [p.name, p.path]));
  const openProject = (name: string) => {
    const path = pathByName.get(name);
    if (path) onSelectProject(path);
  };

  return (
    <div className="animate-fade-in space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold tracking-tight">Overview</h1>
        <Button variant="outline" size="sm" className="gap-1.5" onClick={load} disabled={loading}>
          <RefreshCw className={`h-4 w-4 ${loading ? "animate-spin" : ""}`} />
          Rescan
        </Button>
      </div>

      <div className="grid grid-cols-3 gap-5">
        <StatCard icon={<FolderGit2 className="h-6 w-6" />} label="Projects" value={projects.length} />
        <StatCard icon={<Network className="h-6 w-6" />} label="Listening ports" value={ports.length} />
        <StatCard
          icon={<AlertTriangle className="h-6 w-6" />}
          label="Conflicts"
          value={conflictCount}
          tone="danger"
        />
      </div>

      <Card>
        <CardHeader className="flex flex-row items-center justify-between gap-2 space-y-0">
          <CardTitle className="text-lg">Ports by project</CardTitle>
          <PortMapLegend />
        </CardHeader>
        <CardContent>
          {ports.length === 0 ? (
            <div className="flex flex-col items-center gap-3 py-12 text-center">
              <Network className="h-8 w-8 text-muted-foreground" />
              <p className="text-sm text-muted-foreground">No listening ports found.</p>
            </div>
          ) : (
            <PortMap ports={ports} groupByProject onProjectClick={openProject} />
          )}
        </CardContent>
      </Card>
    </div>
  );
}
