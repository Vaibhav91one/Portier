import { AlertCircle, Check } from "lucide-react";

interface ConflictBadgeProps {
  is_conflict: boolean;
}

export default function ConflictBadge({ is_conflict }: ConflictBadgeProps) {
  if (is_conflict) {
    return (
      <span className="inline-flex items-center gap-1 rounded-md bg-destructive px-2 py-0.5 text-xs font-semibold text-destructive-foreground">
        <AlertCircle className="h-3 w-3" />
        Conflict
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1 rounded-md bg-success px-2 py-0.5 text-xs font-semibold text-success-foreground">
      <Check className="h-3 w-3" />
      OK
    </span>
  );
}
