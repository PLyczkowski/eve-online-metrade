import { useEffect, useMemo, useRef, useState } from "react";
import { CheckCircle2, DatabaseZap, Play, RefreshCw, RotateCcw, Search, Settings, Square, X } from "lucide-react";
import { OpportunityTable } from "./OpportunityTable";
import { api } from "../services/api";
import type { ApiLimitStatus, DiscoverySummary, Filters, Opportunity, Product, RefreshJob, RefreshRun, Setting } from "../types";

const emptyFilters: Filters = { search: "", status: "ALL", direction: "ALL" };

export function App() {
  const [opportunities, setOpportunities] = useState<Opportunity[]>([]);
  const [products, setProducts] = useState<Product[]>([]);
  const [settings, setSettings] = useState<Setting[]>([]);
  const [runs, setRuns] = useState<RefreshRun[]>([]);
  const [refreshJob, setRefreshJob] = useState<RefreshJob | null>(null);
  const [discovery, setDiscovery] = useState<DiscoverySummary | null>(null);
  const [apiLimit, setApiLimit] = useState<ApiLimitStatus | null>(null);
  const [filters, setFilters] = useState<Filters>(emptyFilters);
  const [intervalDraft, setIntervalDraft] = useState("600");
  const [busy, setBusy] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [message, setMessage] = useState("Ready");
  const busyRef = useRef(false);
  const intervalEditingRef = useRef(false);
  const lastJobStatusRef = useRef<string | null>(null);

  async function load() {
    const [opportunityRows, productRows, settingRows, runRows, discoverySummary, apiLimitStatus, jobStatus] = await Promise.all([
      api.listOpportunities(filters),
      api.listProducts(),
      api.listSettings(),
      api.listRefreshRuns(),
      api.listDiscoverySummary(),
      api.listApiLimitStatus(),
      api.getRefreshStatus()
    ]);
    setOpportunities(opportunityRows);
    setProducts(productRows);
    setSettings(settingRows);
    setRuns(runRows);
    setDiscovery(discoverySummary);
    setApiLimit(apiLimitStatus);
    setRefreshJob(jobStatus);
    if (!intervalEditingRef.current) {
      setIntervalDraft(settingValue(settingRows, "Automatic refresh interval seconds") || "600");
    }
  }

  useEffect(() => {
    load().catch((error) => setMessage((error as Error).message));
  }, [filters.status, filters.direction, filters.search]);

  const statusOptions = useMemo(() => {
    const values = new Set(opportunities.map((row) => row.status));
    return ["ALL", ...Array.from(values).sort()];
  }, [opportunities]);

  const latestRun = runs[0];
  const enabledCount = products.filter((product) => product.enabled).length;
  const automaticEnabled = settingValue(settings, "Automatic refresh enabled") !== "FALSE";
  const automaticIntervalSeconds = Math.max(60, Number(settingValue(settings, "Automatic refresh interval seconds")) || 600);
  const refreshRunning = refreshJob?.status === "running";
  const apiBurn = useMemo(
    () => estimateApiBurn(runs, automaticIntervalSeconds, Number(settingValue(settings, "Estimated safe ESI calls per hour")) || 1200, apiLimit),
    [runs, automaticIntervalSeconds, settings, apiLimit]
  );

  useEffect(() => {
    if (!automaticEnabled) return;
    const timer = window.setInterval(() => {
      if (busyRef.current || refreshRunning) return;
      startRefreshJob("Automatic refresh", api.startRefreshNextBatch);
    }, automaticIntervalSeconds * 1000);
    return () => window.clearInterval(timer);
  }, [automaticEnabled, automaticIntervalSeconds, refreshRunning]);

  useEffect(() => {
    const timer = window.setInterval(async () => {
      try {
        const [jobStatus, apiLimitStatus, runRows] = await Promise.all([
          api.getRefreshStatus(),
          api.listApiLimitStatus(),
          api.listRefreshRuns()
        ]);
        setRefreshJob(jobStatus);
        setApiLimit(apiLimitStatus);
        setRuns(runRows);
        const previous = lastJobStatusRef.current;
        lastJobStatusRef.current = jobStatus.status;
        if (previous === "running" && jobStatus.status !== "running") {
          await load();
          setMessage(jobStatus.status === "failed" ? jobStatus.lastError || "Refresh failed" : "Refresh complete");
        }
      } catch (error) {
        setMessage((error as Error).message);
      }
    }, 1500);
    return () => window.clearInterval(timer);
  }, [filters.status, filters.direction, filters.search]);

  async function runAction(label: string, action: () => Promise<unknown>) {
    if (busyRef.current) return;
    busyRef.current = true;
    setBusy(true);
    setMessage(`${label}...`);
    try {
      await action();
      await load();
      setMessage(`${label} complete`);
    } catch (error) {
      setMessage((error as Error).message);
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  }

  async function startRefreshJob(label: string, action: () => Promise<RefreshJob>) {
    if (refreshJob?.status === "running") return;
    setMessage(`${label} started`);
    try {
      const job = await action();
      setRefreshJob(job);
      lastJobStatusRef.current = job.status;
    } catch (error) {
      setMessage((error as Error).message);
    }
  }

  async function refreshRow(typeId: number) {
    await startRefreshJob(`Updating ${typeId}`, () => api.startRefreshProduct(typeId));
  }

  async function editNotes(typeId: number, current: string) {
    const notes = window.prompt("Notes", current);
    if (notes === null) return;
    await runAction("Saved notes", () => api.updateProductNotes(typeId, notes));
  }

  async function disableProduct(typeId: number) {
    await runAction(`Disabled ${typeId}`, () => api.setProductEnabled(typeId, false));
  }

  async function toggleAutomaticRefresh() {
    await runAction("Updated automatic refresh", () => api.updateSetting("Automatic refresh enabled", automaticEnabled ? "FALSE" : "TRUE"));
  }

  async function saveRefreshInterval() {
    const seconds = Math.max(60, Math.round(Number(intervalDraft) || 600));
    setIntervalDraft(seconds.toString());
    await runAction("Updated refresh interval", () => api.updateSetting("Automatic refresh interval seconds", seconds.toString()));
  }

  async function discoverHotProducts() {
    await runAction("Discovered hot products", api.discoverHotProducts);
  }

  return (
    <main className="app-shell">
      <header className="topbar">
        <div>
          <h1>EVE Metrade</h1>
          <p>Jita and Amarr hauling opportunities from public ESI market data.</p>
        </div>
        <div className="topbar-actions">
          <button className="icon-button text-button" onClick={() => startRefreshJob("Refresh next batch", api.startRefreshNextBatch)} disabled={refreshRunning}>
            <Play size={16} /> Refresh next batch
          </button>
          <button className="icon-button text-button" onClick={() => startRefreshJob("Reset refresh", api.startResetAndRefresh)} disabled={refreshRunning}>
            <RotateCcw size={16} /> Reset and refresh
          </button>
          <button className="icon-button text-button" onClick={discoverHotProducts} disabled={busy}>
            <DatabaseZap size={16} /> Discover hot items
          </button>
          <button className="icon-button" onClick={() => setSettingsOpen(true)} title="Settings">
            <Settings size={18} />
          </button>
        </div>
      </header>

      <section className="summary-band">
        <SummaryCard label="Rows" value={opportunities.length.toString()} />
        <SummaryCard label="Enabled products" value={enabledCount.toString()} />
        <SummaryCard label="Known items" value={discovery ? compactNumber(discovery.knownItems) : "0"} />
        <SummaryCard label="Hot candidates" value={discovery ? compactNumber(discovery.candidates) : "0"} />
        <SummaryCard label="Automatic refresh" value={automaticEnabled ? "ON" : "OFF"} icon={automaticEnabled ? <CheckCircle2 size={16} /> : <Square size={16} />} />
        <SummaryCard label="Refresh interval" value={`${automaticIntervalSeconds}s`} />
        <SummaryCard label="Refresh job" value={refreshRunning ? `${refreshJob.scannedCount}/${refreshJob.totalCount || "?"}` : refreshJob?.status ?? "idle"} />
        <SummaryCard label="Last run" value={latestRun ? latestRun.refreshTime : "None"} />
        <SummaryCard label="API calls" value={latestRun ? latestRun.apiCalls.toString() : "0"} />
      </section>

      <section className="api-burn-band" aria-label="API burn rate">
        <div className="api-burn-copy">
          <strong>ESI burn rate</strong>
          <span>{apiBurn.detail}</span>
        </div>
        <div className="api-burn-track" title={apiBurn.label}>
          <div className={`api-burn-fill ${apiBurn.level}`} style={{ width: `${Math.min(100, apiBurn.percent)}%` }} />
        </div>
        <span className={`api-burn-label ${apiBurn.level}`}>{apiBurn.label}</span>
      </section>

      <section className="toolbar" aria-label="Table controls">
        <label className="search-box">
          <Search size={16} />
          <input
            value={filters.search}
            onChange={(event) => setFilters((current) => ({ ...current, search: event.target.value }))}
            placeholder="Search item, type ID, notes"
          />
        </label>
        <select value={filters.status} onChange={(event) => setFilters((current) => ({ ...current, status: event.target.value }))}>
          {statusOptions.map((status) => <option key={status}>{status}</option>)}
        </select>
        <select value={filters.direction} onChange={(event) => setFilters((current) => ({ ...current, direction: event.target.value }))}>
          <option>ALL</option>
          <option>{"Jita -> Amarr"}</option>
          <option>{"Amarr -> Jita"}</option>
        </select>
        <button className="icon-button text-button" onClick={() => setFilters(emptyFilters)}>
          <X size={16} /> Clear filters
        </button>
        <label className="toggle-control">
          <input type="checkbox" checked={automaticEnabled} onChange={toggleAutomaticRefresh} disabled={busy} />
          <span>Automatic refresh</span>
        </label>
        <label className="interval-control">
          <span>Every</span>
          <input
            type="number"
            min={60}
            step={60}
            value={intervalDraft}
            onChange={(event) => setIntervalDraft(event.target.value)}
            onFocus={() => {
              intervalEditingRef.current = true;
            }}
            onBlur={() => {
              intervalEditingRef.current = false;
              saveRefreshInterval();
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter") event.currentTarget.blur();
            }}
            disabled={busy}
          />
          <span>sec</span>
        </label>
        <button className="icon-button" onClick={load} title="Reload table" disabled={busy}>
          <RefreshCw size={17} />
        </button>
      </section>

      <OpportunityTable
        rows={opportunities}
        onRefreshRow={refreshRow}
        onEditNotes={editNotes}
        onDisableProduct={disableProduct}
      />

      <footer className="status-bar">
        <span>{refreshRunning ? refreshProgressText(refreshJob) : busy ? "Working" : message}</span>
        {latestRun?.errors ? <strong>{latestRun.errors}</strong> : null}
      </footer>

      {settingsOpen ? (
        <SettingsPanel
          settings={settings}
          onClose={() => setSettingsOpen(false)}
          onSave={(key, value) => runAction("Saved setting", () => api.updateSetting(key, value))}
        />
      ) : null}
    </main>
  );
}

function SummaryCard({ label, value, icon }: { label: string; value: string; icon?: React.ReactNode }) {
  return (
    <div className="summary-card">
      <span>{label}</span>
      <strong>{icon}{value}</strong>
    </div>
  );
}

function SettingsPanel({ settings, onClose, onSave }: { settings: Setting[]; onClose: () => void; onSave: (key: string, value: string) => Promise<void> }) {
  const [draft, setDraft] = useState<Record<string, string>>(() => Object.fromEntries(settings.map((setting) => [setting.key, setting.value])));
  return (
    <div className="modal-backdrop" role="dialog" aria-modal="true">
      <section className="settings-panel">
        <header>
          <h2>Settings</h2>
          <button className="icon-button" onClick={onClose} title="Close"><X size={18} /></button>
        </header>
        <div className="settings-list">
          {settings.map((setting) => (
            <label key={setting.key}>
              <span>{setting.key}</span>
              <input
                value={draft[setting.key] ?? ""}
                onChange={(event) => setDraft((current) => ({ ...current, [setting.key]: event.target.value }))}
                onBlur={() => onSave(setting.key, draft[setting.key] ?? "")}
              />
              <small>{setting.notes}</small>
            </label>
          ))}
        </div>
      </section>
    </div>
  );
}

function settingValue(settings: Setting[], key: string): string {
  return settings.find((setting) => setting.key === key)?.value ?? "";
}

function compactNumber(value: number): string {
  return Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 }).format(value);
}

function estimateApiBurn(runs: RefreshRun[], intervalSeconds: number, safeBudget: number, apiLimit: ApiLimitStatus | null) {
  if (apiLimit?.rateLimited || (apiLimit?.retryAfter ?? 0) > 0) {
    return {
      callsPerHour: 0,
      percent: 100,
      safeBudget,
      level: "danger",
      label: "Limited",
      detail: `Rate limited; retry after ${apiLimit?.retryAfter ?? apiLimit?.errorLimitReset ?? "?"}s`
    };
  }
  if (apiLimit?.rateLimitRemaining !== null && apiLimit?.rateLimitRemaining !== undefined) {
    const used = apiLimit.rateLimitUsed ?? 0;
    const remaining = apiLimit.rateLimitRemaining;
    const total = used + remaining;
    const percent = total > 0 ? (used / total) * 100 : 0;
    const level = remaining <= Math.max(5, total * 0.15) ? "danger" : remaining <= Math.max(10, total * 0.35) ? "warn" : "ok";
    const label = level === "danger" ? "Low bucket" : level === "warn" ? "Watch" : "Healthy";
    return {
      callsPerHour: 0,
      percent,
      safeBudget,
      level,
      label,
      detail: `Bucket remaining: ${remaining}${apiLimit.rateLimitLimit ? ` of ${apiLimit.rateLimitLimit}` : ""}`
    };
  }
  if (apiLimit?.errorLimitRemain !== null && apiLimit?.errorLimitRemain !== undefined) {
    const remain = apiLimit.errorLimitRemain;
    const usedPercent = Math.max(0, Math.min(100, ((100 - remain) / 100) * 100));
    const level = remain <= 20 ? "danger" : remain <= 40 ? "warn" : "ok";
    const label = level === "danger" ? "Low budget" : level === "warn" ? "Watch" : "Healthy";
    return {
      callsPerHour: 0,
      percent: usedPercent,
      safeBudget,
      level,
      label,
      detail: `Error budget remaining: ${remain}/100; resets in ${apiLimit.errorLimitReset ?? "?"}s`
    };
  }
  const validRuns = runs
    .filter((run) => run.apiCalls > 0)
    .map((run) => ({ ...run, time: Date.parse(run.refreshTime) }))
    .filter((run) => Number.isFinite(run.time))
    .sort((a, b) => b.time - a.time);
  let callsPerHour = 0;
  if (validRuns.length >= 2) {
    const latest = validRuns[0].time;
    const cutoff = latest - 60 * 60 * 1000;
    const recent = validRuns.filter((run) => run.time >= cutoff);
    if (recent.length >= 2) {
      const oldest = recent[recent.length - 1].time;
      const hours = Math.max((latest - oldest) / 3600000, intervalSeconds / 3600);
      callsPerHour = recent.reduce((sum, run) => sum + run.apiCalls, 0) / hours;
    }
  }
  if (callsPerHour === 0 && validRuns[0]) {
    callsPerHour = validRuns[0].apiCalls * (3600 / intervalSeconds);
  }
  const percent = safeBudget > 0 ? (callsPerHour / safeBudget) * 100 : 0;
  const level = percent >= 85 ? "danger" : percent >= 60 ? "warn" : "ok";
  const label = level === "danger" ? "High" : level === "warn" ? "Moderate" : "Low";
  return {
    callsPerHour,
    percent,
    safeBudget,
    level,
    label,
    detail: `${callsPerHour.toFixed(0)} calls/hour estimate, ${percent.toFixed(0)}% of ${safeBudget}/hour budget`
  };
}

function refreshProgressText(job: RefreshJob | null): string {
  if (!job) return "Refreshing...";
  const total = job.totalCount || "?";
  const current = job.currentItem ? ` - ${job.currentItem}` : "";
  return `Refreshing ${job.kind}... ${job.scannedCount}/${total}, ${job.apiCalls} API calls${current}`;
}
