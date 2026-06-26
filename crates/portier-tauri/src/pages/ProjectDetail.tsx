import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowLeft, Check, Loader2, X } from "lucide-react";
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
import type { PortProjectMap, ProjectEntry, ServiceEntry } from "../types";

interface ProjectDetailProps {
  projectPath: string;
  onBack: () => void;
}

export default function ProjectDetail({ projectPath, onBack }: ProjectDetailProps) {
  const [project, setProject] = useState<ProjectEntry | null>(null);
  const [conflictPorts, setConflictPorts] = useState<Set<number>>(new Set());
  const [loading, setLoading] = useState(true);
  const [editService, setEditService] = useState<string | null>(null);
  const [editPort, setEditPort] = useState("");

  const load = () => {
    setLoading(true);
    Promise.all([
      invoke<ProjectEntry>("get_project_detail", { path_str: projectPath }),
      invoke<PortProjectMap[]>("get_port_project_map").catch(() => [] as PortProjectMap[]),
    ])
      .then(([proj, ports]) => {
        setProject(proj);
        setConflictPorts(new Set(ports.filter((p) => p.is_conflict).map((p) => p.port)));
      })
      .catch(() => setProject(null))
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    load();
  }, [projectPath]);

  const handleUpdate = (service: string) => {
    const port = parseInt(editPort, 10);
    if (isNaN(port)) return;
    invoke("assign_port", { project_path: projectPath, service, port })
      .then(() => {
        setEditService(null);
        setEditPort("");
        load();
      })
      .catch(console.error);
  };

  const back = (
    <Button variant="ghost" size="sm" onClick={onBack} className="-ml-2 gap-2">
      <ArrowLeft className="h-4 w-4" />
      Back
    </Button>
  );

  if (loading && !project) {
    return (
      <div className="space-y-4">
        {back}
        <div className="flex h-48 items-center justify-center text-muted-foreground">
          <Loader2 className="h-6 w-6 animate-spin" />
        </div>
      </div>
    );
  }

  if (!project) {
    return (
      <div className="space-y-4">
        {back}
        <p className="text-muted-foreground">Project not found.</p>
      </div>
    );
  }

  const services = Object.entries(project.services) as [string, ServiceEntry][];

  return (
    <div className="animate-fade-in space-y-6">
      {back}

      <Card>
        <CardHeader className="pb-3">
          <div className="flex items-center gap-3">
            <CardTitle className="text-lg">{project.name}</CardTitle>
            <Badge variant="secondary">{project.stack}</Badge>
          </div>
        </CardHeader>
        <CardContent className="space-y-1 text-sm text-muted-foreground">
          <p className="break-all font-mono text-xs">{projectPath}</p>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-base">Services</CardTitle>
        </CardHeader>
        <CardContent className="p-0">
          {services.length === 0 ? (
            <p className="px-6 pb-6 text-sm text-muted-foreground">
              No services configured. Run <code className="rounded bg-muted px-1">portier config init</code>{" "}
              in the project to add ports.
            </p>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="pl-6">Service</TableHead>
                  <TableHead>Preferred</TableHead>
                  <TableHead>Assigned</TableHead>
                  <TableHead>PID</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead className="text-right pr-6">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {services.map(([name, svc]) => (
                  <TableRow key={name}>
                    <TableCell className="pl-6 font-medium">{name}</TableCell>
                    <TableCell className="tnum text-muted-foreground">{svc.preferred}</TableCell>
                    <TableCell className="tnum">
                      {editService === name ? (
                        <input
                          type="number"
                          autoFocus
                          value={editPort}
                          onChange={(e) => setEditPort(e.target.value)}
                          onKeyDown={(e) => e.key === "Enter" && handleUpdate(name)}
                          className="w-20 rounded-md border bg-background px-2 py-1 text-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                        />
                      ) : (
                        <span className="font-medium">{svc.assigned}</span>
                      )}
                    </TableCell>
                    <TableCell className="tnum text-muted-foreground">{svc.pid ?? "—"}</TableCell>
                    <TableCell>
                      <ConflictBadge is_conflict={conflictPorts.has(svc.assigned)} />
                    </TableCell>
                    <TableCell className="pr-6 text-right">
                      {editService === name ? (
                        <div className="flex justify-end gap-1">
                          <Button size="icon" className="h-7 w-7" onClick={() => handleUpdate(name)}>
                            <Check className="h-4 w-4" />
                          </Button>
                          <Button
                            size="icon"
                            variant="ghost"
                            className="h-7 w-7"
                            onClick={() => setEditService(null)}
                          >
                            <X className="h-4 w-4" />
                          </Button>
                        </div>
                      ) : (
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() => {
                            setEditService(name);
                            setEditPort(String(svc.assigned));
                          }}
                        >
                          Edit
                        </Button>
                      )}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
