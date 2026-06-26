import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CheckCircle2, FolderPlus } from "lucide-react";
import { Button } from "../components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "../components/ui/card";
import { Badge } from "../components/ui/badge";
import type { ProjectSummary } from "../types";

interface OnboardingProps {
  onDone: () => void;
}

export default function Onboarding({ onDone }: OnboardingProps) {
  const [pathInput, setPathInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ProjectSummary | null>(null);

  const handleLink = () => {
    if (!pathInput.trim()) return;
    setLoading(true);
    setError(null);
    setResult(null);
    invoke<ProjectSummary>("add_project", { path_str: pathInput.trim() })
      .then((proj) => {
        setResult(proj);
        setPathInput("");
      })
      .catch((e: unknown) => setError(String(e)))
      .finally(() => setLoading(false));
  };

  return (
    <div className="animate-fade-in mx-auto max-w-lg">
      <Card>
        <CardHeader>
          <div className="mb-1 flex h-10 w-10 items-center justify-center rounded-lg bg-primary/10 text-primary">
            <FolderPlus className="h-5 w-5" />
          </div>
          <CardTitle>Add a project</CardTitle>
          <CardDescription>
            Point Portier at a project folder. It detects the stack and tracks its ports.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-1.5">
            <label className="text-sm font-medium">Project path</label>
            <input
              type="text"
              autoFocus
              placeholder="/Users/you/code/my-app"
              value={pathInput}
              onChange={(e) => setPathInput(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleLink()}
              className="w-full rounded-md border bg-background px-3 py-2 text-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
            />
            <p className="text-xs text-muted-foreground">
              The absolute path to the project's root directory.
            </p>
          </div>

          <Button
            onClick={handleLink}
            disabled={loading || !pathInput.trim()}
            className="w-full"
          >
            {loading ? "Linking…" : "Link project"}
          </Button>

          {error && (
            <p className="rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive">
              {error}
            </p>
          )}

          {result && (
            <div className="space-y-3 rounded-md border bg-success/5 p-4">
              <div className="flex items-center gap-2 text-success">
                <CheckCircle2 className="h-5 w-5" />
                <span className="font-medium text-foreground">Linked {result.name}</span>
                <Badge variant="secondary">{result.stack}</Badge>
              </div>
              <Button variant="outline" onClick={onDone} className="w-full">
                Back to Dashboard
              </Button>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
