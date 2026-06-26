import { useEffect, useState } from "react";
import { Moon, Plug, Plus, Sun } from "lucide-react";
import Dashboard from "./pages/Dashboard";
import ProjectDetail from "./pages/ProjectDetail";
import Onboarding from "./pages/Onboarding";
import { Button } from "./components/ui/button";

type View =
  | { page: "dashboard" }
  | { page: "project"; projectPath: string }
  | { page: "onboarding" };

export default function App() {
  const [view, setView] = useState<View>({ page: "dashboard" });
  const [dark, setDark] = useState(false);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", dark);
  }, [dark]);

  const goDashboard = () => setView({ page: "dashboard" });
  const goOnboarding = () => setView({ page: "onboarding" });

  return (
    <div className="min-h-screen bg-background text-foreground">
      <header className="sticky top-0 z-10 border-b bg-background/80 backdrop-blur">
        <div className="flex h-14 items-center justify-between px-8">
          <button
            onClick={goDashboard}
            className="flex items-center gap-2 transition-opacity hover:opacity-80"
          >
            <span className="flex h-7 w-7 items-center justify-center rounded-md bg-primary text-primary-foreground">
              <Plug className="h-4 w-4" />
            </span>
            <span className="text-base font-bold tracking-tight">Portier</span>
            <span className="hidden text-xs text-muted-foreground sm:inline">
              Never fight a port conflict again
            </span>
          </button>

          <nav className="flex items-center gap-1">
            <Button
              variant={view.page === "onboarding" ? "secondary" : "ghost"}
              size="sm"
              className="gap-1.5"
              onClick={goOnboarding}
            >
              <Plus className="h-4 w-4" />
              Add Project
            </Button>
            <Button
              variant="ghost"
              size="icon"
              aria-label="Toggle dark mode"
              onClick={() => setDark((v) => !v)}
            >
              {dark ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
            </Button>
          </nav>
        </div>
      </header>

      <main className="px-8 py-8">
        {view.page === "dashboard" && (
          <Dashboard onSelectProject={(p) => setView({ page: "project", projectPath: p })} />
        )}
        {view.page === "project" && (
          <ProjectDetail projectPath={view.projectPath} onBack={goDashboard} />
        )}
        {view.page === "onboarding" && <Onboarding onDone={goDashboard} />}
      </main>
    </div>
  );
}
