import type { PortProjectMap } from "../types";

interface PortMapProps {
  ports: PortProjectMap[];
  /** Show ports grouped by project instead of a single flat grid */
  groupByProject?: boolean;
  /** Called when a project group header is clicked (registered projects only) */
  onProjectClick?: (projectName: string) => void;
}

type Kind = "conflict" | "assigned" | "process" | "free";

function kindOf(p: PortProjectMap): Kind {
  if (p.is_conflict) return "conflict";
  if (p.project_name) return "assigned";
  if (p.process_names.length > 0) return "process";
  return "free";
}

const KIND_CLASS: Record<Kind, string> = {
  conflict: "bg-destructive text-destructive-foreground",
  assigned: "bg-primary text-primary-foreground",
  process: "bg-muted text-muted-foreground",
  free: "bg-muted/40 text-muted-foreground",
};

const LEGEND: { kind: Kind; label: string }[] = [
  { kind: "conflict", label: "Conflict" },
  { kind: "assigned", label: "Portier project" },
  { kind: "process", label: "Other process" },
  { kind: "free", label: "Free" },
];

export function PortMapLegend() {
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-1.5 text-xs text-muted-foreground">
      {LEGEND.map(({ kind, label }) => (
        <span key={kind} className="flex items-center gap-1.5">
          <span className={`h-3 w-3 rounded-sm ${KIND_CLASS[kind]}`} />
          {label}
        </span>
      ))}
    </div>
  );
}

function portTooltip(p: PortProjectMap): string {
  const parts: string[] = [`Port ${p.port}`];
  if (p.project_name) parts.push(`Project: ${p.project_name}`);
  if (p.service_name) parts.push(`Service: ${p.service_name}`);
  if (p.process_names.length > 0) parts.push(`Process: ${p.process_names.join(", ")}`);
  if (p.is_conflict) parts.push("⚠ CONFLICT");
  return parts.join("\n");
}

function PortCell({ p }: { p: PortProjectMap }) {
  return (
    <div
      title={portTooltip(p)}
      className={`flex aspect-square flex-col items-center justify-center rounded-md p-1 text-center transition-transform hover:scale-105 ${KIND_CLASS[kindOf(p)]}`}
    >
      <span className="tnum text-xs font-semibold leading-none">{p.port}</span>
      {p.project_name && (
        <span className="mt-0.5 max-w-full truncate text-[9px] leading-tight opacity-90">
          {p.project_name}
        </span>
      )}
    </div>
  );
}

function PortGrid({ ports }: { ports: PortProjectMap[] }) {
  return (
    <div className="grid grid-cols-[repeat(auto-fill,minmax(56px,1fr))] gap-2">
      {ports.map((p) => (
        <PortCell key={p.port} p={p} />
      ))}
    </div>
  );
}

export default function PortMap({
  ports,
  groupByProject = false,
  onProjectClick,
}: PortMapProps) {
  if (ports.length === 0) return null;

  if (!groupByProject) {
    return <PortGrid ports={ports} />;
  }

  const groups = ports.reduce(
    (acc, p) => {
      const key = p.project_name ?? "__unassigned__";
      if (!acc[key]) acc[key] = { projectName: p.project_name, ports: [] };
      acc[key].ports.push(p);
      return acc;
    },
    {} as Record<string, { projectName: string | null; ports: PortProjectMap[] }>
  );

  const sorted = Object.entries(groups).sort(([ka], [kb]) => {
    if (ka === "__unassigned__") return 1;
    if (kb === "__unassigned__") return -1;
    return ka.localeCompare(kb);
  });

  return (
    <div className="space-y-4">
      {sorted.map(([key, group]) => {
        const conflicts = group.ports.filter((p) => p.is_conflict).length;
        return (
          <div key={key} className="rounded-lg bg-muted/20 p-4">
            <div className="mb-3 flex items-center gap-2">
              {group.projectName && onProjectClick ? (
                <button
                  onClick={() => onProjectClick(group.projectName!)}
                  className="text-sm font-semibold text-foreground hover:text-primary hover:underline"
                >
                  {group.projectName}
                </button>
              ) : (
                <h4 className="text-sm font-semibold">{group.projectName ?? "Unassigned"}</h4>
              )}
              <span className="text-xs text-muted-foreground">
                {group.ports.length} port{group.ports.length !== 1 ? "s" : ""}
              </span>
              {conflicts > 0 && (
                <span className="rounded-full bg-destructive/10 px-2 py-0.5 text-xs font-medium text-destructive">
                  {conflicts} conflict{conflicts !== 1 ? "s" : ""}
                </span>
              )}
            </div>
            <PortGrid ports={group.ports} />
          </div>
        );
      })}
    </div>
  );
}
