import { useCallback, useEffect, useState } from "react";
import { doctorScan } from "../lib/api";
import type { DoctorResultDto, Scope, ScopeSelection } from "../types";
import { Badge, Button, Card } from "../ui";

interface Props {
  scope: ScopeSelection;
  projectPath?: string;
  hasProject?: boolean;
}

function mergeDoctor(parts: DoctorResultDto[]): DoctorResultDto {
  return {
    duplicates: parts.flatMap((p) => p.duplicates),
    brokenSymlinks: parts.flatMap((p) => p.brokenSymlinks),
    mvSplit: parts.flatMap((p) => p.mvSplit),
    mvStaleLock: parts.flatMap((p) => p.mvStaleLock),
    fixed: parts.reduce((a, p) => a + p.fixed, 0),
  };
}

export default function DoctorView({ scope, projectPath, hasProject }: Props) {
  const [res, setRes] = useState<DoctorResultDto | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [fixing, setFixing] = useState(false);

  const effectiveScopes: Scope[] =
    scope === "all"
      ? hasProject
        ? ["global", "project", "local"]
        : ["global"]
      : (scope === "project" || scope === "local") && !hasProject
        ? []
        : [scope as Scope];

  const scan = useCallback(
    async (fix: boolean) => {
      if (fix) setFixing(true);
      else setLoading(true);
      setErr(null);
      try {
        if (effectiveScopes.length === 0) {
          setRes({ duplicates: [], brokenSymlinks: [], mvSplit: [], mvStaleLock: [], fixed: 0 });
          return;
        }
        const parts = await Promise.all(
          effectiveScopes.map((s) =>
            doctorScan(s, fix, s === "global" ? undefined : projectPath),
          ),
        );
        setRes(mergeDoctor(parts));
      } catch (e) {
        setErr(String(e));
      } finally {
        setLoading(false);
        setFixing(false);
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [scope, projectPath, hasProject],
  );

  useEffect(() => {
    scan(false);
  }, [scan]);

  if (loading) {
    return <div className="p-4 text-sm text-fg-muted">Scanning…</div>;
  }
  if (err) {
    return (
      <div className="m-4 rounded-md border border-danger/40 bg-danger-subtle p-3 text-sm text-danger-fg">
        {err}
      </div>
    );
  }
  if (!res) return null;

  const canFix = res.mvStaleLock.length > 0;
  const nothingWrong =
    res.duplicates.length === 0 &&
    res.brokenSymlinks.length === 0 &&
    res.mvSplit.length === 0 &&
    res.mvStaleLock.length === 0;

  return (
    <div className="p-4">
      <div className="mb-4 flex items-center justify-between">
        <div className="text-sm text-fg-muted">
          Scope: <span className="font-mono">{scope}</span>
          {res.fixed > 0 && (
            <Badge color="success" className="ml-3">
              fixed {res.fixed}
            </Badge>
          )}
        </div>
        <Button
          variant="primary"
          size="sm"
          onClick={() => scan(true)}
          disabled={!canFix || fixing}
        >
          {fixing ? "Fixing…" : "Fix auto-reconcilable issues"}
        </Button>
      </div>

      {nothingWrong && (
        <div className="rounded-md border border-success/40 bg-success-subtle p-3 text-sm text-success-fg">
          All clean.
        </div>
      )}

      {res.duplicates.length > 0 && (
        <Section title={`Duplicates (${res.duplicates.length})`}>
          {res.duplicates.map((d) => (
            <Card key={`${d.unitType}/${d.name}`} className="mb-2 p-2">
              <div className="font-mono text-sm text-fg-default">
                {d.unitType}/{d.name}
              </div>
              <ul className="mt-1 text-xs text-fg-muted">
                {d.locations.map((l, i) => (
                  <li key={i} className="font-mono">
                    [{l.agent}] ({l.scope}) {l.path}
                  </li>
                ))}
              </ul>
            </Card>
          ))}
        </Section>
      )}

      {res.brokenSymlinks.length > 0 && (
        <Section title={`Broken symlinks (${res.brokenSymlinks.length})`}>
          <ul className="font-mono text-xs text-fg-default">
            {res.brokenSymlinks.map((s, i) => (
              <li key={i} className="border-t border-border-default py-1">
                {s}
              </li>
            ))}
          </ul>
        </Section>
      )}

      {res.mvSplit.length > 0 && (
        <Section title={`Mv split-state (${res.mvSplit.length})`}>
          <ul className="font-mono text-xs text-fg-default">
            {res.mvSplit.map((s, i) => (
              <li key={i} className="border-t border-border-default py-1">
                {s}
              </li>
            ))}
          </ul>
        </Section>
      )}

      {res.mvStaleLock.length > 0 && (
        <Section title={`Stale lockfile entries (${res.mvStaleLock.length})`}>
          <ul className="font-mono text-xs text-fg-default">
            {res.mvStaleLock.map((s, i) => (
              <li key={i} className="border-t border-border-default py-1">
                {s}
              </li>
            ))}
          </ul>
        </Section>
      )}
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="mb-4">
      <h3 className="mb-2 text-sm font-semibold text-fg-default">{title}</h3>
      {children}
    </div>
  );
}
