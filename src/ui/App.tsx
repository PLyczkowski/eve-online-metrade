import { CSSProperties, Dispatch, DragEvent, SetStateAction, useEffect, useMemo, useRef, useState } from "react";
import {
  ColumnDef,
  ColumnOrderState,
  ColumnSizingState,
  VisibilityState,
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  SortingState,
  useReactTable
} from "@tanstack/react-table";
import { Bell, CheckCircle2, DatabaseZap, LogIn, Play, RefreshCw, RotateCcw, Search, Settings, Square, X } from "lucide-react";
import { OpportunityTable } from "./OpportunityTable";
import { api } from "../services/api";
import { formatIsk } from "../domain/format";
import type { AccountFilters, ApiLimitStatus, AuthCharacter, AuthEvent, CharacterOrder, DiscoverySummary, Filters, Opportunity, OurOrder, Product, RefreshJob, RefreshRun, SaleNotification, Setting, TradeHub, WalletTransaction } from "../types";

const emptyFilters: Filters = { search: "", status: "ALL", direction: "ALL" };
const emptyAccountFilters: AccountFilters = { search: "", characterId: null, station: "", undercutOnly: false, unknownCostOnly: false, side: "ALL" };
const filterStorageKey = "eve-metrade-filters-v1";
const toolbarSettingKeys = new Set([
  "Automatic refresh enabled",
  "Automatic refresh interval seconds",
  "Max items per refresh",
  "Account refresh enabled",
  "Account refresh interval seconds"
]);

export function App() {
  const [activeTab, setActiveTab] = useState<"opportunities" | "orders" | "transactions">("opportunities");
  const [opportunities, setOpportunities] = useState<Opportunity[]>([]);
  const [ourOrders, setOurOrders] = useState<OurOrder[]>([]);
  const [transactions, setTransactions] = useState<WalletTransaction[]>([]);
  const [saleNotifications, setSaleNotifications] = useState<SaleNotification[]>([]);
  const [products, setProducts] = useState<Product[]>([]);
  const [tradeHubs, setTradeHubs] = useState<TradeHub[]>([]);
  const [settings, setSettings] = useState<Setting[]>([]);
  const [runs, setRuns] = useState<RefreshRun[]>([]);
  const [refreshJob, setRefreshJob] = useState<RefreshJob | null>(null);
  const [discovery, setDiscovery] = useState<DiscoverySummary | null>(null);
  const [apiLimit, setApiLimit] = useState<ApiLimitStatus | null>(null);
  const [authCharacters, setAuthCharacters] = useState<AuthCharacter[]>([]);
  const [authEvents, setAuthEvents] = useState<AuthEvent[]>([]);
  const [characterOrders, setCharacterOrders] = useState<CharacterOrder[]>([]);
  const [filters, setFilters] = useState<Filters>(() => readSavedFilters());
  const [accountFilters, setAccountFilters] = useState<AccountFilters>(emptyAccountFilters);
  const [intervalDraft, setIntervalDraft] = useState("600");
  const [maxItemsDraft, setMaxItemsDraft] = useState("5");
  const [accountIntervalDraft, setAccountIntervalDraft] = useState("3600");
  const [busy, setBusy] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [message, setMessage] = useState("Ready");
  const busyRef = useRef(false);
  const intervalEditingRef = useRef(false);
  const maxItemsEditingRef = useRef(false);
  const accountIntervalEditingRef = useRef(false);
  const lastJobStatusRef = useRef<string | null>(null);
  const notifiedSaleIdsRef = useRef<Set<number>>(new Set());

  async function load() {
    const [opportunityRows, productRows, hubRows, settingRows, runRows, discoverySummary, apiLimitStatus, authRows, authEventRows, orderRows, jobStatus] = await Promise.all([
      api.listOpportunities(filters),
      api.listProducts(),
      api.listTradeHubs(),
      api.listSettings(),
      api.listRefreshRuns(),
      api.listDiscoverySummary(),
      api.listApiLimitStatus(),
      api.listAuthCharacters(),
      api.listAuthEvents(),
      api.listCharacterOrders(),
      api.getRefreshStatus()
    ]);
    setOpportunities(opportunityRows);
    setProducts(productRows);
    setTradeHubs(hubRows);
    setSettings(settingRows);
    setRuns(runRows);
    setDiscovery(discoverySummary);
    setApiLimit(apiLimitStatus);
    setAuthCharacters(authRows);
    setAuthEvents(authEventRows);
    setCharacterOrders(orderRows);
    setRefreshJob(jobStatus);
    if (!intervalEditingRef.current) {
      setIntervalDraft(settingValue(settingRows, "Automatic refresh interval seconds") || "600");
    }
    if (!maxItemsEditingRef.current) {
      setMaxItemsDraft(settingValue(settingRows, "Max items per refresh") || "5");
    }
    if (!accountIntervalEditingRef.current) {
      setAccountIntervalDraft(settingValue(settingRows, "Account refresh interval seconds") || "3600");
    }
    await loadAccountData();
  }

  async function loadAccountData() {
    try {
      const [accountOrderRows, transactionRows, notificationRows] = await Promise.all([
        api.listOurOrders(accountFilters),
        api.listTransactions(accountFilters),
        api.listSaleNotifications()
      ]);
      setOurOrders(accountOrderRows);
      setTransactions(transactionRows);
      setSaleNotifications(notificationRows);
    } catch (error) {
      setMessage(`Account data load failed: ${(error as Error).message}`);
    }
  }

  useEffect(() => {
    load().catch((error) => setMessage((error as Error).message));
  }, [filters.status, filters.direction, filters.search]);

  useEffect(() => {
    localStorage.setItem(filterStorageKey, JSON.stringify(filters));
  }, [filters]);

  useEffect(() => {
    loadAccountData();
  }, [accountFilters.search, accountFilters.characterId, accountFilters.station, accountFilters.undercutOnly, accountFilters.unknownCostOnly, accountFilters.side]);

  const statusOptions = useMemo(() => {
    const values = new Set(opportunities.map((row) => row.status));
    return ["ALL", ...Array.from(values).sort()];
  }, [opportunities]);

  const latestRun = runs[0];
  const enabledCount = products.filter((product) => product.enabled).length;
  const enabledHubCount = tradeHubs.filter((hub) => hub.enabled).length;
  const automaticEnabled = settingValue(settings, "Automatic refresh enabled") !== "FALSE";
  const automaticIntervalSeconds = Math.max(60, Number(settingValue(settings, "Automatic refresh interval seconds")) || 600);
  const accountRefreshEnabled = settingValue(settings, "Account refresh enabled") !== "FALSE";
  const accountIntervalSeconds = Math.max(3600, Number(settingValue(settings, "Account refresh interval seconds")) || 3600);
  const refreshRunning = refreshJob?.status === "running";
  const unseenSales = saleNotifications.filter((row) => !row.seen);
  const apiBurn = useMemo(
    () => estimateApiBurn(runs, automaticIntervalSeconds, Number(settingValue(settings, "Estimated safe ESI calls per hour")) || 1200, apiLimit),
    [runs, automaticIntervalSeconds, settings, apiLimit]
  );

  useEffect(() => {
    if (!automaticEnabled) return;
    const timer = window.setInterval(() => {
      if (busyRef.current || refreshRunning) return;
      startRefreshJob("Automatic refresh", api.startAutoRefreshNextBatch);
    }, automaticIntervalSeconds * 1000);
    return () => window.clearInterval(timer);
  }, [automaticEnabled, automaticIntervalSeconds, refreshRunning]);

  useEffect(() => {
    if (!accountRefreshEnabled) return;
    if (!authCharacters.length) return;
    const timer = window.setInterval(() => {
      if (busyRef.current) return;
      refreshAccountData().catch((error) => setMessage((error as Error).message));
    }, accountIntervalSeconds * 1000);
    return () => window.clearInterval(timer);
  }, [accountRefreshEnabled, authCharacters.length, accountIntervalSeconds]);

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

  useEffect(() => {
    for (const sale of unseenSales) {
      if (notifiedSaleIdsRef.current.has(sale.id)) continue;
      notifiedSaleIdsRef.current.add(sale.id);
      if ("Notification" in window && Notification.permission === "granted") {
        new Notification("EVE Metrade sale", {
          body: `${sale.quantity} x ${sale.itemName} @ ${formatIsk(sale.unitPrice)}`
        });
      } else if ("Notification" in window && Notification.permission === "default") {
        Notification.requestPermission().catch(() => undefined);
      }
    }
  }, [unseenSales]);

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

  async function startRefreshJob(label: string, action: () => Promise<RefreshJob>, allowQueue = false) {
    if (refreshJob?.status === "running" && !allowQueue) return;
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
    await startRefreshJob(`Queued update for ${typeId}`, () => api.startRefreshProduct(typeId), true);
  }

  async function refreshRows(typeIds: number[]) {
    const uniqueTypeIds = Array.from(new Set(typeIds));
    for (const typeId of uniqueTypeIds) {
      await api.startRefreshProduct(typeId);
    }
    const job = await api.getRefreshStatus();
    setRefreshJob(job);
    lastJobStatusRef.current = job.status;
    setMessage(`Queued ${uniqueTypeIds.length} selected updates`);
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

  async function toggleAccountRefresh() {
    await runAction("Updated account refresh", () => api.updateSetting("Account refresh enabled", accountRefreshEnabled ? "FALSE" : "TRUE"));
  }

  async function saveRefreshInterval() {
    const seconds = Math.max(60, Math.round(Number(intervalDraft) || 600));
    setIntervalDraft(seconds.toString());
    await runAction("Updated refresh interval", () => api.updateSetting("Automatic refresh interval seconds", seconds.toString()));
  }

  async function saveMaxItemsPerRefresh() {
    const count = Math.max(1, Math.round(Number(maxItemsDraft) || 5));
    setMaxItemsDraft(count.toString());
    await runAction("Updated items per refresh", () => api.updateSetting("Max items per refresh", count.toString()));
  }

  async function saveAccountRefreshInterval() {
    const seconds = Math.max(3600, Math.round(Number(accountIntervalDraft) || 3600));
    setAccountIntervalDraft(seconds.toString());
    await runAction("Updated account refresh interval", () => api.updateSetting("Account refresh interval seconds", seconds.toString()));
  }

  async function discoverHotProducts() {
    await runAction("Discovered hot products", api.discoverHotProducts);
  }

  async function toggleTradeHub(id: number, enabled: boolean) {
    await runAction("Updated trade hub", () => api.setTradeHubEnabled(id, enabled));
  }

  async function startEveLogin() {
    await runAction("EVE login", api.startEveLogin);
  }

  async function refreshOrders(characterId: number) {
    await runAction("Refreshed character orders", () => api.refreshCharacterOrders(characterId));
  }

  async function refreshAccountData(characterId?: number) {
    const ids = characterId ? [characterId] : authCharacters.map((character) => character.characterId);
    if (!ids.length) {
      setMessage("Log in with EVE first.");
      return;
    }
    await runAction("Refreshed account data", async () => {
      for (const id of ids) {
        await api.refreshAccountData(id);
      }
    });
  }

  async function editOrderCost(order: OurOrder) {
    const unit = window.prompt("Bought for / unit", order.boughtUnitPrice?.toString() ?? "");
    if (unit === null) return;
    const quantity = window.prompt("Bought quantity matched", (order.boughtQuantityMatched ?? order.volumeTotal).toString());
    if (quantity === null) return;
    await runAction("Saved order cost", () => api.updateOrderCostBasis(order.orderId, Number(unit), Math.round(Number(quantity))));
  }

  async function markSalesSeen() {
    await runAction("Cleared sale notifications", () => api.markSaleNotificationsSeen(unseenSales.map((sale) => sale.id)));
  }

  async function clearApiError() {
    const next = await api.clearApiError();
    setApiLimit(next);
    setMessage("Cleared ESI request error");
  }

  return (
    <main className="app-shell">
      <header className="topbar">
        <div className="topbar-title">
          <h1>EVE Metrade</h1>
          <p>Trade hub hauling opportunities from public ESI market data.</p>
        </div>
        <div className="topbar-actions">
          <button className="icon-button text-button" onClick={() => startRefreshJob("Refresh next batch", api.startRefreshNextBatch)} disabled={refreshRunning}>
            <Play size={16} /> Refresh batch
          </button>
          <button className="icon-button text-button" onClick={() => startRefreshJob("Reset refresh", api.startResetAndRefresh)} disabled={refreshRunning}>
            <RotateCcw size={16} /> Reset refresh
          </button>
          <button className="icon-button text-button" onClick={discoverHotProducts} disabled={busy}>
            <DatabaseZap size={16} /> Discover
          </button>
          <button className="icon-button" onClick={() => setSettingsOpen(true)} title="Settings">
            <Settings size={18} />
          </button>
        </div>
      </header>

      <nav className="tabbar" aria-label="Main sections">
        <button className={activeTab === "opportunities" ? "active" : ""} onClick={() => setActiveTab("opportunities")}>Opportunities</button>
        <button className={activeTab === "orders" ? "active" : ""} onClick={() => setActiveTab("orders")}>Our Orders ({ourOrders.length})</button>
        <button className={activeTab === "transactions" ? "active" : ""} onClick={() => setActiveTab("transactions")}>Transactions ({transactions.length})</button>
        {unseenSales.length ? (
          <button className="sale-badge" onClick={markSalesSeen} title="Mark sale notifications seen">
            <Bell size={15} /> {unseenSales.length} new sale{unseenSales.length === 1 ? "" : "s"}
          </button>
        ) : null}
      </nav>

      <section className="summary-band">
        <SummaryCard label="Rows" value={opportunities.length.toString()} />
        <SummaryCard label="Enabled products" value={enabledCount.toString()} />
        <SummaryCard label="Enabled hubs" value={enabledHubCount.toString()} />
        <SummaryCard label="Characters" value={authCharacters.length.toString()} />
        <SummaryCard label="Known items" value={discovery ? compactNumber(discovery.knownItems) : "0"} />
        <SummaryCard label="Hot candidates" value={discovery ? compactNumber(discovery.candidates) : "0"} />
        <SummaryCard label="Automatic refresh" value={automaticEnabled ? "ON" : "OFF"} icon={automaticEnabled ? <CheckCircle2 size={16} /> : <Square size={16} />} />
        <SummaryCard label="Refresh interval" value={`${automaticIntervalSeconds}s`} />
        <SummaryCard label="Account refresh" value={accountRefreshEnabled ? `${accountIntervalSeconds}s` : "OFF"} />
        <SummaryCard label="Refresh job" value={refreshRunning ? `${refreshJob.scannedCount}/${refreshJob.totalCount || "?"}` : refreshJob?.status ?? "idle"} />
        <SummaryCard label="Last run" value={latestRun ? latestRun.refreshTime : "None"} />
      </section>

      <section className="api-burn-band" aria-label="API burn rate">
        <div className="api-burn-copy">
          <strong>ESI burn rate</strong>
          <span>{apiBurn.detail}</span>
        </div>
        <div className="api-burn-track" title={apiBurn.label}>
          <div className={`api-burn-fill ${apiBurn.level}`} style={{ width: `${Math.min(100, apiBurn.percent)}%` }} />
        </div>
        <div className="api-burn-actions">
          <span className={`api-burn-label ${apiBurn.level}`}>{apiBurn.label}</span>
          {apiLimit?.lastError ? (
            <button className="link-button" onClick={clearApiError} disabled={busy}>
              Clear
            </button>
          ) : null}
        </div>
      </section>

      {activeTab === "opportunities" ? <section className="toolbar" aria-label="Table controls">
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
          {directionOptions(opportunities).map((direction) => <option key={direction}>{direction}</option>)}
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
        <label className="interval-control">
          <span>Items</span>
          <input
            type="number"
            min={1}
            step={1}
            value={maxItemsDraft}
            onChange={(event) => setMaxItemsDraft(event.target.value)}
            onFocus={() => {
              maxItemsEditingRef.current = true;
            }}
            onBlur={() => {
              maxItemsEditingRef.current = false;
              saveMaxItemsPerRefresh();
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter") event.currentTarget.blur();
            }}
            disabled={busy}
          />
          <span>/ refresh</span>
        </label>
        <button className="icon-button" onClick={load} title="Reload table" disabled={busy}>
          <RefreshCw size={17} />
        </button>
      </section> : (
        <AccountToolbar
          filters={accountFilters}
          characters={authCharacters}
          rows={ourOrders}
          transactions={transactions}
          activeTab={activeTab}
          onFiltersChange={setAccountFilters}
          onRefreshAccount={() => refreshAccountData()}
          accountRefreshEnabled={accountRefreshEnabled}
          accountIntervalDraft={accountIntervalDraft}
          onToggleAccountRefresh={toggleAccountRefresh}
          onAccountIntervalDraftChange={setAccountIntervalDraft}
          onAccountIntervalFocus={() => {
            accountIntervalEditingRef.current = true;
          }}
          onAccountIntervalBlur={() => {
            accountIntervalEditingRef.current = false;
            saveAccountRefreshInterval();
          }}
          busy={busy}
        />
      )}

      {activeTab === "opportunities" ? (
        <OpportunityTable
          rows={opportunities}
          onRefreshRow={refreshRow}
          onRefreshRows={refreshRows}
          onEditNotes={editNotes}
          onDisableProduct={disableProduct}
        />
      ) : activeTab === "orders" ? (
        <OrdersTable rows={ourOrders} onEditCost={editOrderCost} />
      ) : (
        <TransactionsTable rows={transactions} />
      )}

      <footer className="status-bar">
        <span>{refreshRunning ? refreshProgressText(refreshJob) : busy ? "Working" : message}</span>
        {latestRun?.errors ? <strong>{latestRun.errors}</strong> : null}
      </footer>

      {settingsOpen ? (
        <SettingsPanel
          settings={settings.filter((setting) => !toolbarSettingKeys.has(setting.key))}
          tradeHubs={tradeHubs}
          authCharacters={authCharacters}
          authEvents={authEvents}
          characterOrders={characterOrders}
          onClose={() => setSettingsOpen(false)}
          onSave={(key, value) => runAction("Saved setting", () => api.updateSetting(key, value))}
          onToggleHub={toggleTradeHub}
          onStartEveLogin={startEveLogin}
          onRefreshOrders={refreshOrders}
        />
      ) : null}
    </main>
  );
}

function AccountToolbar({
  filters,
  characters,
  rows,
  transactions,
  activeTab,
  onFiltersChange,
  onRefreshAccount,
  accountRefreshEnabled,
  accountIntervalDraft,
  onToggleAccountRefresh,
  onAccountIntervalDraftChange,
  onAccountIntervalFocus,
  onAccountIntervalBlur,
  busy
}: {
  filters: AccountFilters;
  characters: AuthCharacter[];
  rows: OurOrder[];
  transactions: WalletTransaction[];
  activeTab: "orders" | "transactions";
  onFiltersChange: Dispatch<SetStateAction<AccountFilters>>;
  onRefreshAccount: () => Promise<void>;
  accountRefreshEnabled: boolean;
  accountIntervalDraft: string;
  onToggleAccountRefresh: () => Promise<void>;
  onAccountIntervalDraftChange: (value: string) => void;
  onAccountIntervalFocus: () => void;
  onAccountIntervalBlur: () => void;
  busy: boolean;
}) {
  const stations = Array.from(new Set([...rows.map((row) => row.stationName), ...transactions.map((row) => row.stationName)].filter(Boolean))).sort();
  return (
    <section className="toolbar" aria-label="Account controls">
      <label className="search-box">
        <Search size={16} />
        <input
          value={filters.search}
          onChange={(event) => onFiltersChange((current) => ({ ...current, search: event.target.value }))}
          placeholder="Search item, type ID, station"
        />
      </label>
      <select
        value={filters.characterId ?? ""}
        onChange={(event) => onFiltersChange((current) => ({ ...current, characterId: event.target.value ? Number(event.target.value) : null }))}
      >
        <option value="">All characters</option>
        {characters.map((character) => <option key={character.characterId} value={character.characterId}>{character.characterName}</option>)}
      </select>
      <select value={filters.station} onChange={(event) => onFiltersChange((current) => ({ ...current, station: event.target.value }))}>
        <option value="">All stations</option>
        {stations.map((station) => <option key={station}>{station}</option>)}
      </select>
      {activeTab === "transactions" ? (
        <select value={filters.side} onChange={(event) => onFiltersChange((current) => ({ ...current, side: event.target.value }))}>
          <option value="ALL">All</option>
          <option value="SELL">Sell</option>
          <option value="BUY">Buy</option>
        </select>
      ) : (
        <>
          <label className="toggle-control">
            <input type="checkbox" checked={filters.undercutOnly} onChange={(event) => onFiltersChange((current) => ({ ...current, undercutOnly: event.target.checked }))} />
            <span>Undercut only</span>
          </label>
          <label className="toggle-control">
            <input type="checkbox" checked={filters.unknownCostOnly} onChange={(event) => onFiltersChange((current) => ({ ...current, unknownCostOnly: event.target.checked }))} />
            <span>Unknown cost</span>
          </label>
        </>
      )}
      <button className="icon-button text-button" onClick={() => onFiltersChange(emptyAccountFilters)}>
        <X size={16} /> Clear filters
      </button>
      <button className="icon-button text-button" onClick={onRefreshAccount} disabled={busy || characters.length === 0}>
        <RefreshCw size={16} /> Refresh account data
      </button>
      <label className="toggle-control">
        <input type="checkbox" checked={accountRefreshEnabled} onChange={() => onToggleAccountRefresh()} disabled={busy || characters.length === 0} />
        <span>Auto account refresh</span>
      </label>
      <label className="interval-control">
        <span>Every</span>
        <input
          type="number"
          min={3600}
          step={300}
          value={accountIntervalDraft}
          onChange={(event) => onAccountIntervalDraftChange(event.target.value)}
          onFocus={onAccountIntervalFocus}
          onBlur={onAccountIntervalBlur}
          onKeyDown={(event) => {
            if (event.key === "Enter") event.currentTarget.blur();
          }}
          disabled={busy || characters.length === 0}
        />
        <span>sec</span>
      </label>
    </section>
  );
}

function OrdersTable({ rows, onEditCost }: { rows: OurOrder[]; onEditCost: (order: OurOrder) => Promise<void> }) {
  const maxProfitRemain = Math.max(...rows.map((row) => row.expectedProfitRemaining ?? 0), 0);
  const columns = useMemo<ColumnDef<OurOrder>[]>(() => [
    { accessorKey: "characterName", header: "Character", size: 130 },
    { accessorKey: "itemName", header: "Item", size: 230, cell: ({ row }) => <>{row.original.itemName}<small>{row.original.typeId}</small></> },
    { accessorKey: "stationName", header: "Station", size: 120 },
    { id: "quantity", header: "Qty", size: 100, cell: ({ row }) => `${row.original.volumeRemain}/${row.original.volumeTotal}` },
    { accessorKey: "price", header: "Price", size: 115, cell: ({ getValue }) => formatIsk(getValue<number | null>()) },
    { accessorKey: "lowestCompetingPrice", header: "Lowest", size: 115, cell: ({ getValue }) => formatIsk(getValue<number | null>()) },
    { accessorKey: "suggestedUpdatePrice", header: "Update Price", size: 125, cell: ({ getValue }) => formatIsk(getValue<number | null>()) },
    { accessorKey: "estimatedUpdateFee", header: "Update Fee", size: 120, cell: ({ getValue }) => formatIsk(getValue<number | null>()) },
    { accessorKey: "expectedProfitRemaining", header: "Profit Remain", size: 130, cell: ({ getValue }) => formatIsk(getValue<number | null>()) },
    { id: "boughtUnitPrice", header: "Bought / Unit", size: 125, cell: ({ row }) => <button className="link-button" onClick={() => onEditCost(row.original)}>{row.original.boughtUnitPrice === null ? "Set" : `${formatIsk(row.original.boughtUnitPrice)}${row.original.manualCost ? " *" : ""}`}</button> },
    { accessorKey: "boughtQuantityMatched", header: "Matched Qty", size: 115 },
    { accessorKey: "expectedProfitPerUnit", header: "Profit / Unit", size: 120, cell: ({ getValue }) => formatIsk(getValue<number | null>()) },
    { accessorKey: "expiresAt", header: "Expires", size: 125, cell: ({ getValue }) => formatTimeLeft(getValue<string | null>()) },
    { accessorKey: "refreshedAt", header: "Last Refresh", size: 125, cell: ({ getValue }) => minutesAgoLabel(getValue<string | null>()) }
  ], [onEditCost]);
  return (
    <AccountDataTable
      tableId="orders"
      rows={rows}
      columns={columns}
      emptyMessage="No active sell orders loaded. Use Refresh account data, or re-login if wallet/order scope changed."
      getCellStyle={(columnId, row) => {
        if (columnId === "price" && row.isUndercut) return { backgroundColor: "#fde2e2", fontWeight: 650 };
        if (columnId === "stationName") return stationCellColor(row.stationName);
        if (columnId === "expectedProfitRemaining") return { backgroundColor: greenScale(row.expectedProfitRemaining ?? 0, 0, maxProfitRemain) };
        if (columnId === "quantity") return quantityFill(row.volumeRemain, row.volumeTotal);
        if (columnId === "refreshedAt") return { backgroundColor: refreshScale(minutesAgo(row.refreshedAt)) };
        return {};
      }}
    />
  );
}

function TransactionsTable({ rows }: { rows: WalletTransaction[] }) {
  const columns = useMemo<ColumnDef<WalletTransaction>[]>(() => [
    { accessorKey: "transactionDate", header: "Date", size: 135, cell: ({ getValue }) => shortDate(getValue<string | null>()) },
    { id: "side", header: "Side", size: 80, cell: ({ row }) => row.original.isBuy ? "Buy" : "Sell" },
    { accessorKey: "itemName", header: "Item", size: 250, cell: ({ row }) => <>{row.original.itemName}<small>{row.original.typeId}</small></> },
    { accessorKey: "stationName", header: "Station", size: 120 },
    { accessorKey: "quantity", header: "Qty", size: 90 },
    { accessorKey: "unitPrice", header: "Unit Price", size: 120, cell: ({ getValue }) => formatIsk(getValue<number | null>()) },
    { accessorKey: "totalPrice", header: "Total", size: 130, cell: ({ getValue }) => formatIsk(getValue<number | null>()) },
    { accessorKey: "matchedOrderId", header: "Matched Order", size: 130 }
  ], []);
  return (
    <AccountDataTable
      tableId="transactions"
      rows={rows}
      columns={columns}
      emptyMessage="No wallet transactions loaded. Use Refresh account data, and re-login if the wallet scope was just added."
      getRowClass={(row) => row.isBuy ? "is-buy" : "is-sell"}
    />
  );
}

function AccountDataTable<T>({
  tableId,
  rows,
  columns,
  emptyMessage,
  getRowClass,
  getCellStyle
}: {
  tableId: string;
  rows: T[];
  columns: ColumnDef<T>[];
  emptyMessage: string;
  getRowClass?: (row: T) => string;
  getCellStyle?: (columnId: string, row: T) => CSSProperties;
}) {
  const [sorting, setSorting] = useState<SortingState>([]);
  const [columnSizing, setColumnSizing] = useState<ColumnSizingState>(() => readSavedTableState(`eve-metrade-${tableId}-column-sizing-v1`, {}));
  const [columnOrder, setColumnOrder] = useState<ColumnOrderState>(() => readSavedTableState(`eve-metrade-${tableId}-column-order-v1`, []));
  const [columnVisibility, setColumnVisibility] = useState<VisibilityState>(() => readSavedTableState(`eve-metrade-${tableId}-column-visibility-v1`, {}));
  const [headerMenu, setHeaderMenu] = useState<{ x: number; y: number } | null>(null);
  const [draggedColumn, setDraggedColumn] = useState<string | null>(null);
  const [dropColumn, setDropColumn] = useState<string | null>(null);
  const defaultColumnOrder = useMemo(() => columns.map((column) => column.id ?? (column as { accessorKey?: string }).accessorKey).filter(Boolean) as string[], [columns]);
  const table = useReactTable({
    data: rows,
    columns,
    state: { sorting, columnSizing, columnOrder: columnOrder.length ? columnOrder : defaultColumnOrder, columnVisibility },
    onSortingChange: setSorting,
    onColumnSizingChange: (updater) => persistTableState(`eve-metrade-${tableId}-column-sizing-v1`, updater, setColumnSizing),
    onColumnOrderChange: (updater) => persistTableState(`eve-metrade-${tableId}-column-order-v1`, updater, setColumnOrder),
    onColumnVisibilityChange: (updater) => persistTableState(`eve-metrade-${tableId}-column-visibility-v1`, updater, setColumnVisibility),
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    columnResizeMode: "onEnd"
  });
  return (
    <section className="account-table-shell" onClick={() => setHeaderMenu(null)}>
      <table className="account-table" style={{ width: table.getTotalSize() }}>
        <thead>
          {table.getHeaderGroups().map((group) => (
            <tr key={group.id}>
              {group.headers.map((header) => (
                <th
                  key={header.id}
                  className={dropColumn === header.column.id ? "column-drop-target" : ""}
                  style={{ width: header.getSize() }}
                  onDragOver={(event) => {
                    event.preventDefault();
                    setDropColumn(header.column.id);
                  }}
                  onDragLeave={() => setDropColumn(null)}
                  onDrop={(event) => {
                    event.preventDefault();
                    moveColumn(table.getAllLeafColumns().map((column) => column.id), draggedColumn, header.column.id, table.setColumnOrder);
                    setDraggedColumn(null);
                    setDropColumn(null);
                  }}
                  onDragEnd={() => {
                    setDraggedColumn(null);
                    setDropColumn(null);
                  }}
                  onContextMenu={(event) => {
                    event.preventDefault();
                    setHeaderMenu({ x: event.clientX, y: event.clientY });
                  }}
                >
                  <button
                    className="column-header-button"
                    draggable
                    onDragStart={(event) => handleHeaderDragStart(event, header.column.id, setDraggedColumn)}
                    onClick={header.column.getToggleSortingHandler()}
                  >
                    {flexRender(header.column.columnDef.header, header.getContext())}
                    <span>{header.column.getIsSorted() === "asc" ? " ▲" : header.column.getIsSorted() === "desc" ? " ▼" : ""}</span>
                  </button>
                  <div
                    className={`column-resizer ${header.column.getIsResizing() ? "is-resizing" : ""}`}
                    style={{
                      transform: header.column.getIsResizing()
                        ? `translateX(${table.getState().columnSizingInfo.deltaOffset ?? 0}px)`
                        : undefined
                    }}
                    onMouseDown={(event) => {
                      event.stopPropagation();
                      header.getResizeHandler()(event);
                    }}
                    onTouchStart={(event) => {
                      event.stopPropagation();
                      header.getResizeHandler()(event);
                    }}
                  />
                </th>
              ))}
            </tr>
          ))}
        </thead>
        <tbody>
          {table.getRowModel().rows.length === 0 ? (
            <tr>
              <td colSpan={table.getVisibleLeafColumns().length || 1} className="empty-table-cell">{emptyMessage}</td>
            </tr>
          ) : null}
          {table.getRowModel().rows.map((row) => (
            <tr key={row.id} className={getRowClass?.(row.original) ?? ""}>
              {row.getVisibleCells().map((cell) => (
                <td key={cell.id} style={{ width: cell.column.getSize(), ...(getCellStyle?.(cell.column.id, row.original) ?? {}) }}>
                  {flexRender(cell.column.columnDef.cell, cell.getContext())}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      {headerMenu ? (
        <ColumnMenu
          x={headerMenu.x}
          y={headerMenu.y}
          columns={table.getAllLeafColumns().map((column) => ({
            id: column.id,
            label: String(column.columnDef.header ?? column.id),
            visible: column.getIsVisible(),
            canHide: column.getCanHide(),
            toggle: () => column.toggleVisibility()
          }))}
          onClose={() => setHeaderMenu(null)}
        />
      ) : null}
    </section>
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

function SettingsPanel({
  settings,
  tradeHubs,
  authCharacters,
  authEvents,
  characterOrders,
  onClose,
  onSave,
  onToggleHub,
  onStartEveLogin,
  onRefreshOrders
}: {
  settings: Setting[];
  tradeHubs: TradeHub[];
  authCharacters: AuthCharacter[];
  authEvents: AuthEvent[];
  characterOrders: CharacterOrder[];
  onClose: () => void;
  onSave: (key: string, value: string) => Promise<void>;
  onToggleHub: (id: number, enabled: boolean) => Promise<void>;
  onStartEveLogin: () => Promise<void>;
  onRefreshOrders: (characterId: number) => Promise<void>;
}) {
  const [draft, setDraft] = useState<Record<string, string>>(() => Object.fromEntries(settings.map((setting) => [setting.key, setting.value])));
  function setDraftValue(key: string, value: string) {
    setDraft((current) => ({ ...current, [key]: value }));
  }
  return (
    <div className="modal-backdrop" role="dialog" aria-modal="true">
      <section className="settings-panel">
        <header>
          <h2>Settings</h2>
          <button className="icon-button" onClick={onClose} title="Close"><X size={18} /></button>
        </header>
        <div className="settings-list">
          <section className="settings-section">
            <h3>Trade hubs</h3>
            <div className="hub-list">
              {tradeHubs.map((hub) => (
                <label key={hub.id} className="hub-row">
                  <input
                    type="checkbox"
                    checked={hub.enabled}
                    onChange={(event) => onToggleHub(hub.id, event.target.checked)}
                  />
                  <span>{hub.name}</span>
                  <small>Region {hub.regionId}, station {hub.stationId}</small>
                </label>
              ))}
            </div>
          </section>
          <section className="settings-section">
            <div className="section-heading-row">
              <h3>EVE login</h3>
              <button className="icon-button text-button" onClick={onStartEveLogin}>
                <LogIn size={16} /> Log in with EVE
              </button>
            </div>
            {authCharacters.length === 0 ? (
              <p className="settings-help">Set the EVE SSO client ID below, then log in.</p>
            ) : (
              <div className="character-list">
                {authCharacters.map((character) => {
                  const orders = characterOrders.filter((order) => order.characterId === character.characterId);
                  const latest = orders[0]?.refreshedAt ?? "Not fetched";
                  return (
                    <div key={character.characterId} className="character-row">
                      <div>
                        <strong>{character.characterName}</strong>
                        <small>{orders.length} active orders, refreshed {latest}</small>
                      </div>
                      <button className="icon-button text-button" onClick={() => onRefreshOrders(character.characterId)}>
                        <RefreshCw size={16} /> Refresh orders
                      </button>
                    </div>
                  );
                })}
              </div>
            )}
            {authEvents.length > 0 ? (
              <div className="auth-event-list">
                {authEvents.slice(0, 5).map((event) => (
                  <span key={`${event.happenedAt}-${event.message}`} className={`auth-event ${event.status}`}>
                    {event.happenedAt}: {event.message}
                  </span>
                ))}
              </div>
            ) : null}
            {characterOrders.length > 0 ? (
              <div className="order-preview">
                {characterOrders.slice(0, 8).map((order) => (
                  <span key={`${order.characterId}-${order.orderId}`}>
                    {order.isBuyOrder ? "Buy" : "Sell"} type {order.typeId}: {order.volumeRemain}/{order.volumeTotal} @ {formatIsk(order.price)}
                  </span>
                ))}
              </div>
            ) : null}
          </section>
          {settings.map((setting) => {
            const value = draft[setting.key] ?? "";
            const kind = settingInputKind(setting);
            return (
              <label key={setting.key}>
                <span>{setting.key}</span>
                {kind === "boolean" ? (
                  <input
                    type="checkbox"
                    checked={value !== "FALSE"}
                    onChange={(event) => {
                      const next = event.target.checked ? "TRUE" : "FALSE";
                      setDraftValue(setting.key, next);
                      onSave(setting.key, next);
                    }}
                  />
                ) : kind === "number" ? (
                  <input
                    type="number"
                    step={numberStep(setting.key, value)}
                    value={value}
                    onChange={(event) => setDraftValue(setting.key, event.target.value)}
                    onBlur={() => onSave(setting.key, draft[setting.key] ?? "")}
                  />
                ) : (
                  <input
                    type={kind === "url" ? "url" : "text"}
                    value={value}
                    onChange={(event) => setDraftValue(setting.key, event.target.value)}
                    onBlur={() => onSave(setting.key, draft[setting.key] ?? "")}
                  />
                )}
                <small>{setting.notes}</small>
              </label>
            );
          })}
        </div>
      </section>
    </div>
  );
}

function directionOptions(rows: Opportunity[]): string[] {
  return Array.from(new Set(rows.map((row) => row.direction).filter(Boolean))).sort();
}

function readSavedFilters(): Filters {
  try {
    const saved = localStorage.getItem(filterStorageKey);
    if (!saved) return emptyFilters;
    const parsed = JSON.parse(saved) as Partial<Filters>;
    return {
      search: parsed.search ?? "",
      status: parsed.status ?? "ALL",
      direction: parsed.direction ?? "ALL"
    };
  } catch {
    return emptyFilters;
  }
}

function settingInputKind(setting: Setting): "boolean" | "number" | "url" | "text" {
  const value = setting.value.trim();
  if (value === "TRUE" || value === "FALSE") return "boolean";
  if (/^-?\d+(\.\d+)?$/.test(value)) return "number";
  if (/^https?:\/\//i.test(value)) return "url";
  return "text";
}

function numberStep(key: string, value: string): string {
  if (key.toLowerCase().includes("spread") || value.includes(".")) return "0.01";
  return "1";
}

function settingValue(settings: Setting[], key: string): string {
  return settings.find((setting) => setting.key === key)?.value ?? "";
}

function compactNumber(value: number): string {
  return Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 }).format(value);
}

function estimateApiBurn(runs: RefreshRun[], intervalSeconds: number, safeBudget: number, apiLimit: ApiLimitStatus | null) {
  if (apiLimit?.lastError) {
    return {
      callsPerHour: 0,
      percent: 100,
      safeBudget,
      level: "danger",
      label: "Request error",
      detail: apiLimit.lastError
    };
  }
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
  const queued = job.queuedCount ? `, ${job.queuedCount} queued` : "";
  const idle = secondsSince(job.lastProgressAt);
  const idleText = idle === null ? "" : `, ${idle}s since progress`;
  return `Refreshing ${job.kind}... ${job.scannedCount}/${total}, ${job.apiCalls} API calls${idleText}${queued}${current}`;
}

function secondsSince(value: string): number | null {
  if (!value) return null;
  const parsed = new Date(value).getTime();
  if (Number.isNaN(parsed)) return null;
  return Math.max(0, Math.round((Date.now() - parsed) / 1000));
}

function shortDate(value: string | null): string {
  if (!value) return "";
  const parsed = new Date(value).getTime();
  if (Number.isNaN(parsed)) return value;
  return new Intl.DateTimeFormat("en", {
    month: "short",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit"
  }).format(parsed);
}

function minutesAgo(value: string | null): number | null {
  if (!value) return null;
  const parsed = new Date(value).getTime();
  if (Number.isNaN(parsed)) return null;
  return Math.max(0, Math.round((Date.now() - parsed) / 60000));
}

function minutesAgoLabel(value: string | null): string {
  const minutes = minutesAgo(value);
  return minutes === null ? "" : `${minutes} min ago`;
}

function formatTimeLeft(value: string | null): string {
  if (!value) return "";
  const parsed = new Date(value).getTime();
  if (Number.isNaN(parsed)) return "";
  let minutes = Math.max(0, Math.round((parsed - Date.now()) / 60000));
  const days = Math.floor(minutes / 1440);
  minutes -= days * 1440;
  const hours = Math.floor(minutes / 60);
  minutes -= hours * 60;
  if (days > 0) return `${days}d ${hours}h ${minutes}m`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  return `${minutes}m`;
}

function stationCellColor(station: string): CSSProperties {
  if (station === "Jita") return { backgroundColor: "#ffedd5" };
  if (station === "Amarr") return { backgroundColor: "#dbeafe" };
  return {};
}

function quantityFill(remaining: number, total: number): CSSProperties {
  if (!total || total <= 0) return {};
  const percent = Math.max(0, Math.min(100, (remaining / total) * 100));
  return {
    background: `linear-gradient(90deg, #dcfce7 0%, #dcfce7 ${percent}%, #ffffff ${percent}%, #ffffff 100%)`
  };
}

function greenScale(value: number, low: number, high: number): string {
  if (!value || value < low || high <= low) return "#ffffff";
  const ratio = Math.min(1, Math.max(0, (value - low) / (high - low)));
  const green = Math.round(245 - ratio * 100);
  const redBlue = Math.round(255 - ratio * 170);
  return `rgb(${redBlue}, ${green}, ${redBlue})`;
}

function refreshScale(minutes: number | null): string {
  if (minutes === null) return "#ffffff";
  if (minutes <= 5) return "#ccf0cc";
  if (minutes >= 30) return "#f4cccc";
  if (minutes <= 17.5) {
    const ratio = (minutes - 5) / 12.5;
    return mixColor([204, 240, 204], [255, 242, 165], ratio);
  }
  const ratio = (minutes - 17.5) / 12.5;
  return mixColor([255, 242, 165], [244, 204, 204], ratio);
}

function mixColor(from: [number, number, number], to: [number, number, number], ratio: number): string {
  const values = from.map((value, index) => Math.round(value + (to[index] - value) * ratio));
  return `rgb(${values[0]}, ${values[1]}, ${values[2]})`;
}

function readSavedTableState<T>(key: string, fallback: T): T {
  try {
    const saved = localStorage.getItem(key);
    return saved ? JSON.parse(saved) as T : fallback;
  } catch {
    return fallback;
  }
}

function persistTableState<T>(key: string, updater: T | ((current: T) => T), setter: Dispatch<SetStateAction<T>>) {
  setter((current) => {
    const next = typeof updater === "function" ? (updater as (current: T) => T)(current) : updater;
    localStorage.setItem(key, JSON.stringify(next));
    return next;
  });
}

function handleHeaderDragStart(event: DragEvent<HTMLElement>, columnId: string, setDraggedColumn: Dispatch<SetStateAction<string | null>>) {
  setDraggedColumn(columnId);
  event.dataTransfer.effectAllowed = "move";
}

function moveColumn(currentOrder: string[], from: string | null, to: string, setColumnOrder: (updater: ColumnOrderState) => void) {
  if (!from || from === to) return;
  const next = currentOrder.filter((id) => id !== from);
  const toIndex = next.indexOf(to);
  next.splice(toIndex < 0 ? next.length : toIndex, 0, from);
  setColumnOrder(next);
}

function ColumnMenu({
  x,
  y,
  columns,
  onClose
}: {
  x: number;
  y: number;
  columns: Array<{ id: string; label: string; visible: boolean; canHide: boolean; toggle: () => void }>;
  onClose: () => void;
}) {
  return (
    <div className="column-menu" style={{ left: x, top: y }} onClick={(event) => event.stopPropagation()}>
      <header>
        <strong>Columns</strong>
        <button onClick={onClose}>Close</button>
      </header>
      {columns.map((column) => (
        <label key={column.id}>
          <input type="checkbox" checked={column.visible} disabled={!column.canHide} onChange={column.toggle} />
          <span>{column.label}</span>
        </label>
      ))}
    </div>
  );
}
