import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  AlertTriangle,
  FolderGit2,
  Loader2,
  Network,
  Plus,
  RefreshCw,
} from "lucide-react";
import { Button } from "../components/ui/button";
import { Badge } from "../components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "../components/ui/table";
import ConflictBadge from "../components/ConflictBadge";
import PortMap, { PortMapLegend } from "../components/PortMap";
import type { PortProjectMap, ProjectSummary } from "../types";

interface DashboardProps {
  onSelectProject: (path: string) => void;
  onAddProject: () => void;
}

/** Two-option segmented toggle — the active option is highlighted. */
function Segmented<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (v: T) => void;
}) {
  return (
    <div className="inline-flex rounded-md border bg-muted/40 p-0.5">
      {options.map((opt) => (
        <button
          key={opt.value}
          onClick={() => onChange(opt.value)}
          className={`rounded px-2.5 py-1 text-xs font-medium transition-colors ${
            value === opt.value
              ? "bg-background text-foreground shadow-sm"
              : "text-muted-foreground hover:text-foreground"
          }`}
        >
          {opt.label}
        </button>
      ))}
    </div>
  );
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
      <CardContent className="flex items-center gap-3 p-4">
        <span
          className={`flex h-10 w-10 items-center justify-center rounded-lg ${
            tone === "danger"
              ? "bg-destructive/10 text-destructive"
              : "bg-primary/10 text-primary"
          }`}
        >
          {icon}
        </span>
        <div>
          <div
            className={`tnum text-2xl font-bold leading-none ${
              tone === "danger" && value > 0 ? "text-destructive" : ""
            }`}
          >
            {value}
          </div>
          <div className="mt-1 text-xs text-muted-foreground">{label}</div>
        </div>
      </CardContent>
    </Card>
  );
}

export default function Dashboard({ onSelectProject, onAddProject }: DashboardProps) {
  const [ports, setPorts] = useState<PortProjectMap[]>([]);
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [portView, setPortView] = useState<"all" | "project">("all");
  const [projView, setProjView] = useState<"flat" | "stack">("flat");
  const [expandedStacks, setExpandedStacks] = useState<Set<string>>(new Set());

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
        setExpandedStacks(new Set(proj.map((p) => p.stack)));
      })
      .catch((e: unknown) => setError(String(e)))
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    load();
  }, []);

  const toggleStack = (stack: string) => {
    setExpandedStacks((prev) => {
      const next = new Set(prev);
      next.has(stack) ? next.delete(stack) : next.add(stack);
      return next;
    });
  };

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
      <div className="flex h-64 flex-col items-center justify-center gap-3 text-muted-foreground">
        <Loader2 className="h-6 w-6 animate-spin" />
        <p className="text-sm">Scanning ports…</p>
      </div>
    );
  }

  const grouped = projects.reduce(
    (acc, p) => {
      (acc[p.stack] = acc[p.stack] || []).push(p);
      return acc;
    },
    {} as Record<string, ProjectSummary[]>
  );

  const conflictCount = ports.filter((p) => p.is_conflict).length;
  const projectConflicts = new Set(
    ports.filter((p) => p.is_conflict && p.project_name).map((p) => p.project_name!)
  );

  const ProjectNameButton = ({ proj }: { proj: ProjectSummary }) => (
    <button
      onClick={() => onSelectProject(proj.path)}
      className="text-left font-medium text-primary underline-offset-4 hover:underline"
    >
      {proj.name}
    </button>
  );

  return (
    <div className="animate-fade-in space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-bold tracking-tight">Overview</h1>
        <Button variant="outline" size="sm" className="gap-1.5" onClick={load} disabled={loading}>
          <RefreshCw className={`h-4 w-4 ${loading ? "animate-spin" : ""}`} />
          Rescan
        </Button>
      </div>

      <div className="grid grid-cols-3 gap-4">
        <StatCard icon={<FolderGit2 className="h-5 w-5" />} label="Projects" value={projects.length} />
        <StatCard icon={<Network className="h-5 w-5" />} label="Listening ports" value={ports.length} />
        <StatCard
          icon={<AlertTriangle className="h-5 w-5" />}
          label="Conflicts"
          value={conflictCount}
          tone="danger"
        />
      </div>

      <Card>
        <CardHeader className="flex flex-row items-center justify-between gap-2 space-y-0">
          <CardTitle className="text-base">Port Map</CardTitle>
          <Segmented
            value={portView}
            onChange={setPortView}
            options={[
              { value: "all", label: "All ports" },
              { value: "project", label: "By project" },
            ]}
          />
        </CardHeader>
        <CardContent className="space-y-4">
          {ports.length === 0 ? (
            <p className="text-sm text-muted-foreground">No listening ports found.</p>
          ) : (
            <>
              <PortMap ports={ports} groupByProject={portView === "project"} />
              <PortMapLegend />
            </>
          )}
        </CardContent>
      </Card>

      <div className="flex items-center justify-between">
        <h2 className="text-base font-semibold">Projects</h2>
        {projects.length > 0 && (
          <Segmented
            value={projView}
            onChange={setProjView}
            options={[
              { value: "flat", label: "Flat" },
              { value: "stack", label: "By stack" },
            ]}
          />
        )}
      </div>

      {projects.length === 0 ? (
        <Card className="border-dashed">
          <CardContent className="flex flex-col items-center gap-3 py-12 text-center">
            <FolderGit2 className="h-8 w-8 text-muted-foreground" />
            <div>
              <p className="font-medium">No projects yet</p>
              <p className="text-sm text-muted-foreground">
                Link a project so Portier can manage its ports.
              </p>
            </div>
            <Button size="sm" className="gap-1.5" onClick={onAddProject}>
              <Plus className="h-4 w-4" />
              Add Project
            </Button>
          </CardContent>
        </Card>
      ) : projView === "stack" ? (
        <div className="space-y-3">
          {Object.entries(grouped).map(([stack, stackProjects]) => {
            const expanded = expandedStacks.has(stack);
            return (
              <Card key={stack}>
                <CardHeader
                  className="cursor-pointer select-none py-3"
                  onClick={() => toggleStack(stack)}
                >
                  <CardTitle className="flex items-center gap-2 text-sm">
                    <span className="w-3 text-muted-foreground">{expanded ? "▾" : "▸"}</span>
                    <Badge variant="secondary">{stack}</Badge>
                    <span className="font-normal text-muted-foreground">
                      {stackProjects.length} project{stackProjects.length !== 1 ? "s" : ""}
                    </span>
                  </CardTitle>
                </CardHeader>
                {expanded && (
                  <CardContent>
                    <Table>
                      <TableHeader>
                        <TableRow>
                          <TableHead>Name</TableHead>
                          <TableHead>Services</TableHead>
                          <TableHead>Status</TableHead>
                        </TableRow>
                      </TableHeader>
                      <TableBody>
                        {stackProjects.map((proj) => (
                          <TableRow key={proj.path}>
                            <TableCell>
                              <ProjectNameButton proj={proj} />
                            </TableCell>
                            <TableCell className="tnum">{proj.services_count}</TableCell>
                            <TableCell>
                              <ConflictBadge is_conflict={projectConflicts.has(proj.name)} />
                            </TableCell>
                          </TableRow>
                        ))}
                      </TableBody>
                    </Table>
                  </CardContent>
                )}
              </Card>
            );
          })}
        </div>
      ) : (
        <Card>
          <CardContent className="p-0">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="pl-4">Name</TableHead>
                  <TableHead>Stack</TableHead>
                  <TableHead>Services</TableHead>
                  <TableHead>Status</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {projects.map((proj) => (
                  <TableRow key={proj.path}>
                    <TableCell className="pl-4">
                      <ProjectNameButton proj={proj} />
                    </TableCell>
                    <TableCell>
                      <Badge variant="secondary">{proj.stack}</Badge>
                    </TableCell>
                    <TableCell className="tnum">{proj.services_count}</TableCell>
                    <TableCell>
                      <ConflictBadge is_conflict={projectConflicts.has(proj.name)} />
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
