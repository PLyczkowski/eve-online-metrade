import type { AccountFilters, AccountRefreshResult, ApiLimitStatus, AuthCharacter, AuthEvent, CharacterOrder, DiscoveryRun, DiscoverySummary, Filters, Opportunity, OurOrder, Product, RefreshJob, RefreshRun, SaleNotification, Setting, TradeHub, WalletTransaction } from "../types";
import { seedOpportunities, seedProducts, seedRefreshRuns, seedSettings } from "../data/seed";
import { analyzeOpportunity, defaultMarketConfig, shouldSkipLowTargetVolume } from "../domain/market";

type Command =
  | "list_opportunities"
  | "list_products"
  | "list_trade_hubs"
  | "list_settings"
  | "list_refresh_runs"
  | "list_discovery_summary"
  | "list_api_limit_status"
  | "list_auth_characters"
  | "list_auth_events"
  | "start_eve_login"
  | "refresh_character_orders"
  | "list_character_orders"
  | "refresh_account_data"
  | "list_our_orders"
  | "list_transactions"
  | "list_sale_notifications"
  | "update_order_cost_basis"
  | "mark_sale_notifications_seen"
  | "get_refresh_status"
  | "discover_hot_products"
  | "start_refresh_next_batch"
  | "start_reset_and_refresh"
  | "start_refresh_product"
  | "refresh_next_batch"
  | "refresh_product"
  | "reset_and_refresh"
  | "update_product_notes"
  | "update_setting"
  | "set_trade_hub_enabled"
  | "set_product_enabled";

interface StoreShape {
  products: Product[];
  settings: Setting[];
  tradeHubs: TradeHub[];
  opportunities: Opportunity[];
  refreshRuns: RefreshRun[];
  refreshJob: RefreshJob;
  authCharacters: AuthCharacter[];
  authEvents: AuthEvent[];
  characterOrders: CharacterOrder[];
  walletTransactions: WalletTransaction[];
  saleNotifications: SaleNotification[];
  cursor: number;
}

const storeKey = "eve-metrade-store-v1";

async function call<T>(command: Command, args?: Record<string, unknown>): Promise<T> {
  if (isTauri()) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(command, args);
  }
  return fallbackCommand<T>(command, args);
}

function isTauri(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export const api = {
  listOpportunities(filters: Filters) {
    return call<Opportunity[]>("list_opportunities", { filters });
  },
  listProducts() {
    return call<Product[]>("list_products");
  },
  listTradeHubs() {
    return call<TradeHub[]>("list_trade_hubs");
  },
  listSettings() {
    return call<Setting[]>("list_settings");
  },
  listRefreshRuns() {
    return call<RefreshRun[]>("list_refresh_runs");
  },
  listDiscoverySummary() {
    return call<DiscoverySummary>("list_discovery_summary");
  },
  listApiLimitStatus() {
    return call<ApiLimitStatus>("list_api_limit_status");
  },
  listAuthCharacters() {
    return call<AuthCharacter[]>("list_auth_characters");
  },
  listAuthEvents() {
    return call<AuthEvent[]>("list_auth_events");
  },
  startEveLogin() {
    return call<AuthCharacter>("start_eve_login");
  },
  refreshCharacterOrders(characterId: number) {
    return call<CharacterOrder[]>("refresh_character_orders", { characterId });
  },
  listCharacterOrders(characterId?: number) {
    return call<CharacterOrder[]>("list_character_orders", { characterId });
  },
  refreshAccountData(characterId: number) {
    return call<AccountRefreshResult>("refresh_account_data", { characterId });
  },
  listOurOrders(filters: AccountFilters) {
    return call<OurOrder[]>("list_our_orders", { filters });
  },
  listTransactions(filters: AccountFilters) {
    return call<WalletTransaction[]>("list_transactions", { filters });
  },
  listSaleNotifications() {
    return call<SaleNotification[]>("list_sale_notifications");
  },
  updateOrderCostBasis(orderId: number, unitCost: number, quantity: number) {
    return call<void>("update_order_cost_basis", { orderId, unitCost, quantity });
  },
  markSaleNotificationsSeen(ids: number[]) {
    return call<void>("mark_sale_notifications_seen", { ids });
  },
  getRefreshStatus() {
    return call<RefreshJob>("get_refresh_status");
  },
  discoverHotProducts() {
    return call<DiscoveryRun>("discover_hot_products");
  },
  startRefreshNextBatch() {
    return call<RefreshJob>("start_refresh_next_batch");
  },
  startResetAndRefresh() {
    return call<RefreshJob>("start_reset_and_refresh");
  },
  startRefreshProduct(typeId: number) {
    return call<RefreshJob>("start_refresh_product", { typeId });
  },
  refreshNextBatch() {
    return call<RefreshRun>("refresh_next_batch");
  },
  resetAndRefresh() {
    return call<RefreshRun>("reset_and_refresh");
  },
  refreshProduct(typeId: number) {
    return call<Opportunity>("refresh_product", { typeId });
  },
  updateProductNotes(typeId: number, notes: string) {
    return call<Product>("update_product_notes", { typeId, notes });
  },
  updateSetting(key: string, value: string) {
    return call<Setting>("update_setting", { key, value });
  },
  setTradeHubEnabled(id: number, enabled: boolean) {
    return call<TradeHub>("set_trade_hub_enabled", { id, enabled });
  },
  setProductEnabled(typeId: number, enabled: boolean) {
    return call<Product>("set_product_enabled", { typeId, enabled });
  }
};

async function fallbackCommand<T>(command: Command, args?: Record<string, unknown>): Promise<T> {
  const store = readStore();
  if (command === "list_opportunities") return filterOpportunities(mergedOpportunityRows(store), args?.filters as Filters) as T;
  if (command === "list_products") return store.products as T;
  if (command === "list_trade_hubs") return store.tradeHubs as T;
  if (command === "list_settings") return store.settings as T;
  if (command === "list_refresh_runs") return store.refreshRuns.slice().reverse() as T;
  if (command === "list_discovery_summary") return fallbackDiscoverySummary(store) as T;
  if (command === "list_api_limit_status") return fallbackApiLimitStatus() as T;
  if (command === "list_auth_characters") return (store.authCharacters ?? []) as T;
  if (command === "list_auth_events") return (store.authEvents ?? []) as T;
  if (command === "list_character_orders") {
    const characterId = args?.characterId == null ? null : Number(args.characterId);
    const rows = store.characterOrders ?? [];
    return (characterId ? rows.filter((row) => row.characterId === characterId) : rows) as T;
  }
  if (command === "list_transactions") return filterFallbackTransactions(store.walletTransactions ?? [], args?.filters as AccountFilters) as T;
  if (command === "list_our_orders") return [] as T;
  if (command === "list_sale_notifications") return (store.saleNotifications ?? []) as T;
  if (command === "mark_sale_notifications_seen") {
    const ids = new Set((args?.ids as number[]) ?? []);
    store.saleNotifications = (store.saleNotifications ?? []).map((row) => ids.has(row.id) ? { ...row, seen: true } : row);
    writeStore(store);
    return undefined as T;
  }
  if (command === "update_order_cost_basis") return undefined as T;
  if (command === "start_eve_login" || command === "refresh_character_orders" || command === "refresh_account_data") {
    throw new Error("EVE login requires the desktop app.");
  }
  if (command === "get_refresh_status") return store.refreshJob as T;
  if (command === "discover_hot_products") return fallbackDiscoverHotProducts(store) as T;
  if (command === "update_product_notes") return updateProductNotes(store, Number(args?.typeId), String(args?.notes ?? "")) as T;
  if (command === "update_setting") return updateSetting(store, String(args?.key), String(args?.value ?? "")) as T;
  if (command === "set_trade_hub_enabled") return setTradeHubEnabled(store, Number(args?.id), Boolean(args?.enabled)) as T;
  if (command === "set_product_enabled") return setProductEnabled(store, Number(args?.typeId), Boolean(args?.enabled)) as T;
  if (command === "refresh_product") return refreshOneFallback(store, Number(args?.typeId)) as T;
  if (command === "refresh_next_batch") return refreshBatchFallback(store, false) as T;
  if (command === "reset_and_refresh") return refreshBatchFallback(store, true) as T;
  if (command === "start_refresh_product") return startFallbackJob(store, "product", () => refreshOneFallback(store, Number(args?.typeId))) as T;
  if (command === "start_refresh_next_batch") return startFallbackJob(store, "batch", () => refreshBatchFallback(store, false)) as T;
  if (command === "start_reset_and_refresh") return startFallbackJob(store, "reset", () => refreshBatchFallback(store, true)) as T;
  throw new Error(`Unknown command: ${command}`);
}

function readStore(): StoreShape {
  const stored = localStorage.getItem(storeKey);
  if (stored) {
    const store = JSON.parse(stored) as StoreShape;
    if (!store.refreshJob) store.refreshJob = idleJob();
    if (!store.tradeHubs) store.tradeHubs = seedTradeHubs();
    if (!store.authCharacters) store.authCharacters = [];
    if (!store.authEvents) store.authEvents = [];
    if (!store.characterOrders) store.characterOrders = [];
    if (!store.walletTransactions) store.walletTransactions = [];
    if (!store.saleNotifications) store.saleNotifications = [];
    return store;
  }
  const store: StoreShape = {
    products: seedProducts,
    settings: seedSettings,
    tradeHubs: seedTradeHubs(),
    opportunities: seedOpportunities,
    refreshRuns: seedRefreshRuns,
    refreshJob: idleJob(),
    authCharacters: [],
    authEvents: [],
    characterOrders: [],
    walletTransactions: [],
    saleNotifications: [],
    cursor: 0
  };
  writeStore(store);
  return store;
}

function seedTradeHubs(): TradeHub[] {
  return [
    { id: 1, name: "Jita", regionId: 10000002, stationId: 60003760, enabled: true, priority: 1 },
    { id: 2, name: "Amarr", regionId: 10000043, stationId: 60008494, enabled: true, priority: 2 },
    { id: 3, name: "Dodixie", regionId: 10000032, stationId: 60011866, enabled: true, priority: 3 },
    { id: 4, name: "Rens", regionId: 10000030, stationId: 60004588, enabled: false, priority: 4 },
    { id: 5, name: "Hek", regionId: 10000042, stationId: 60005686, enabled: false, priority: 5 }
  ];
}

function writeStore(store: StoreShape) {
  localStorage.setItem(storeKey, JSON.stringify(store));
}

function filterOpportunities(rows: Opportunity[], filters: Filters): Opportunity[] {
  const search = filters.search.trim().toLowerCase();
  return rows
    .filter((row) => (filters.status === "ALL" ? true : row.status === filters.status))
    .filter((row) => (filters.direction === "ALL" ? true : row.direction === filters.direction))
    .filter((row) => {
      if (!search) return true;
      return `${row.typeId} ${row.itemName} ${row.notes} ${row.scriptNotes}`.toLowerCase().includes(search);
    });
}

function mergedOpportunityRows(store: StoreShape): Opportunity[] {
  const rowsByType = new Map(store.opportunities.map((row) => [row.typeId, row]));
  return store.products
    .filter((product) => product.enabled)
    .map((product) => rowsByType.get(product.typeId) ?? pendingOpportunity(product));
}

function pendingOpportunity(product: Product): Opportunity {
  return {
    status: "PENDING",
    direction: "",
    typeId: product.typeId,
    itemName: product.name,
    buyHub: "",
    sellHub: "",
    buyPrice: null,
    sellReference: null,
    destinationLowestSell: null,
    profitPerUnit: null,
    spread: null,
    sourceAvailable: null,
    estimatedProfit: null,
    score: null,
    cargoUsedPercent: null,
    suggestedBuyQuantity: null,
    myDestinationSellPriceMin: null,
    myDestinationSellPriceMax: null,
    myDestinationSellQuantity: null,
    myDestinationSellOrderCount: null,
    buyRegionVolume: null,
    sellRegionVolume: null,
    lastRefresh: null,
    lastRefreshMinutes: null,
    notes: product.notes,
    scriptNotes: "Awaiting ESI validation"
  };
}

function fallbackDiscoverySummary(store: StoreShape): DiscoverySummary {
  return {
    knownItems: store.products.length,
    marketRows: 0,
    candidates: store.products.length,
    products: store.products.length,
    enabledProducts: store.products.filter((product) => product.enabled).length,
    lastDiscovery: "Browser fallback"
  };
}

function fallbackApiLimitStatus(): ApiLimitStatus {
  return {
    lastResponseAt: "Browser fallback",
    lastStatus: 0,
    errorLimitRemain: null,
    errorLimitReset: null,
    retryAfter: null,
    rateLimitLimit: "",
    rateLimitRemaining: null,
    rateLimitUsed: null,
    rateLimited: false,
    lastUrl: ""
  };
}

function filterFallbackTransactions(rows: WalletTransaction[], filters: AccountFilters): WalletTransaction[] {
  const search = filters.search.trim().toLowerCase();
  return rows
    .filter((row) => filters.characterId === null || row.characterId === filters.characterId)
    .filter((row) => !filters.station || row.stationName === filters.station)
    .filter((row) => filters.side === "BUY" ? row.isBuy : filters.side === "SELL" ? !row.isBuy : true)
    .filter((row) => !search || `${row.typeId} ${row.itemName} ${row.stationName}`.toLowerCase().includes(search));
}

function idleJob(): RefreshJob {
  return {
    status: "idle",
    kind: "",
    currentItem: "",
    scannedCount: 0,
    totalCount: 0,
    apiCalls: 0,
    lastError: "",
    queuedCount: 0,
    startedAt: "",
    lastProgressAt: "",
    finishedAt: ""
  };
}

function startFallbackJob(store: StoreShape, kind: string, action: () => Promise<unknown>): RefreshJob {
  store.refreshJob = {
    ...idleJob(),
    status: "running",
    kind,
    startedAt: new Date().toISOString(),
    lastProgressAt: new Date().toISOString()
  };
  writeStore(store);
  action()
    .then(() => {
      const nextStore = readStore();
      nextStore.refreshJob = { ...nextStore.refreshJob, status: "done", finishedAt: new Date().toISOString() };
      writeStore(nextStore);
    })
    .catch((error) => {
      const nextStore = readStore();
      nextStore.refreshJob = {
        ...nextStore.refreshJob,
        status: "failed",
        lastError: (error as Error).message,
        finishedAt: new Date().toISOString()
      };
      writeStore(nextStore);
    });
  return store.refreshJob;
}

function fallbackDiscoverHotProducts(store: StoreShape): DiscoveryRun {
  const run: DiscoveryRun = {
    runTime: new Date().toISOString(),
    itemTypesImported: store.products.length,
    marketRowsImported: 0,
    candidatesFound: store.products.length,
    productsEnabled: store.products.filter((product) => product.enabled).length,
    errors: "Desktop app required for full Fuzzwork import.",
    durationSeconds: 0
  };
  writeStore(store);
  return run;
}

function updateProductNotes(store: StoreShape, typeId: number, notes: string): Product {
  const product = mustFindProduct(store, typeId);
  product.notes = notes;
  const opportunity = store.opportunities.find((row) => row.typeId === typeId);
  if (opportunity) opportunity.notes = notes;
  writeStore(store);
  return product;
}

function updateSetting(store: StoreShape, key: string, value: string): Setting {
  let setting = store.settings.find((row) => row.key === key);
  if (!setting) {
    setting = { key, value, notes: "" };
    store.settings.push(setting);
  }
  setting.value = value;
  writeStore(store);
  return setting;
}

function setTradeHubEnabled(store: StoreShape, id: number, enabled: boolean): TradeHub {
  if (!store.tradeHubs) store.tradeHubs = seedTradeHubs();
  const hub = store.tradeHubs.find((row) => row.id === id);
  if (!hub) throw new Error(`Unknown trade hub ${id}`);
  hub.enabled = enabled;
  writeStore(store);
  return hub;
}

function setProductEnabled(store: StoreShape, typeId: number, enabled: boolean): Product {
  const product = mustFindProduct(store, typeId);
  product.enabled = enabled;
  writeStore(store);
  return product;
}

async function refreshOneFallback(store: StoreShape, typeId: number): Promise<Opportunity> {
  const product = mustFindProduct(store, typeId);
  const old = store.opportunities.find((row) => row.typeId === typeId);
  const minTargetVolume = Number(settingValue(store, "Skip refresh if target 30d volume below", "0"));
  if (shouldSkipLowTargetVolume(old, minTargetVolume)) {
    return old as Opportunity;
  }
  const opportunity = await fetchAndAnalyze(product, store);
  upsertOpportunity(store, opportunity);
  store.refreshRuns.push({
    refreshTime: opportunity.lastRefresh ?? new Date().toISOString(),
    itemsScanned: 1,
    opportunitiesWritten: 1,
    apiCalls: 4,
    errors: "",
    skipped: "Manual product refresh",
    durationSeconds: 0
  });
  writeStore(store);
  return opportunity;
}

async function refreshBatchFallback(store: StoreShape, reset: boolean): Promise<RefreshRun> {
  if (reset) store.cursor = 0;
  const maxItems = Number(settingValue(store, "Max items per refresh", "5"));
  const enabled = store.products.filter((product) => product.enabled);
  const selected = enabled.slice(store.cursor, store.cursor + maxItems);
  const started = Date.now();
  let errors = "";
  let written = 0;
  let skipped = 0;

  for (const product of selected) {
    const old = store.opportunities.find((row) => row.typeId === product.typeId);
    const minTargetVolume = Number(settingValue(store, "Skip refresh if target 30d volume below", "0"));
    if (shouldSkipLowTargetVolume(old, minTargetVolume)) {
      skipped++;
      continue;
    }
    try {
      upsertOpportunity(store, await fetchAndAnalyze(product, store));
      written++;
      await sleep(Number(settingValue(store, "Delay between items ms", "0")));
    } catch (error) {
      errors += `${product.typeId}: ${(error as Error).message}\n`;
    }
  }

  store.cursor += selected.length;
  const complete = store.cursor >= enabled.length;
  if (complete) store.cursor = 0;

  const run: RefreshRun = {
    refreshTime: new Date().toISOString(),
    itemsScanned: selected.length,
    opportunitiesWritten: written,
    apiCalls: selected.length * 4,
    errors: errors.trim(),
    skipped: `${complete ? "Complete" : `Next starts at item ${store.cursor + 1} of ${enabled.length}`}${skipped ? `; skipped low target volume: ${skipped}` : ""}`,
    durationSeconds: Math.round((Date.now() - started) / 1000)
  };
  store.refreshRuns.push(run);
  writeStore(store);
  return run;
}

async function fetchAndAnalyze(product: Product, store: StoreShape): Promise<Opportunity> {
  const baseUrl = settingValue(store, "ESI base URL", "https://esi.evetech.net/latest");
  const forgeRegion = Number(settingValue(store, "The Forge region ID", "10000002"));
  const domainRegion = Number(settingValue(store, "Domain region ID", "10000043"));
  const [forgeOrders, domainOrders, forgeHistory, domainHistory] = await Promise.all([
    fetchJson<any[]>(`${baseUrl}/markets/${forgeRegion}/orders/?datasource=tranquility&order_type=sell&type_id=${product.typeId}&page=1`),
    fetchJson<any[]>(`${baseUrl}/markets/${domainRegion}/orders/?datasource=tranquility&order_type=sell&type_id=${product.typeId}&page=1`),
    fetchJson<any[]>(`${baseUrl}/markets/${forgeRegion}/history/?datasource=tranquility&type_id=${product.typeId}`),
    fetchJson<any[]>(`${baseUrl}/markets/${domainRegion}/history/?datasource=tranquility&type_id=${product.typeId}`)
  ]);
  const config = {
    ...defaultMarketConfig,
    jitaStationId: Number(settingValue(store, "Jita station ID", "60003760")),
    amarrStationId: Number(settingValue(store, "Amarr station ID", "60008494")),
    minimumSpread: Number(settingValue(store, "Minimum spread", "0.2")),
    minimumEstimatedProfit: Number(settingValue(store, "Minimum estimated profit", "500000")),
    minimumSourceVolume: Number(settingValue(store, "Minimum 30d source volume", "1")),
    minimumDestinationVolume: Number(settingValue(store, "Minimum 30d destination volume", "1")),
    sellReferenceMinimumUnits: Number(settingValue(store, "Sell reference minimum units", "5")),
    sellReferenceMinimumIskDepth: Number(settingValue(store, "Sell reference minimum ISK depth", "25000000")),
    shipCargoCapacityM3: Number(settingValue(store, "Ship cargo capacity m3", "7900")),
    suggestedBuyDestinationVolumePercent: Number(settingValue(store, "Suggested buy max destination 30d volume percent", "0.3")),
    scoreTargetProfit: Number(settingValue(store, "Score target profit ISK", "100000000")),
    scoreProfitWeight: Number(settingValue(store, "Score profit weight", "50")),
    scoreSellThroughWeight: Number(settingValue(store, "Score sell-through weight", "40")),
    scoreCargoWeight: Number(settingValue(store, "Score cargo weight", "10"))
  };

  return analyzeOpportunity({
    product,
    config,
    refreshedAt: new Date().toISOString(),
    forgeVolume: recentHistoryVolume(forgeHistory, config.historyDays),
    domainVolume: recentHistoryVolume(domainHistory, config.historyDays),
    forgeOrders: forgeOrders.map(toOrder),
    domainOrders: domainOrders.map(toOrder)
  });
}

async function fetchJson<T>(url: string): Promise<T> {
  const response = await fetch(url);
  if (response.status === 404) return [] as T;
  if (!response.ok) throw new Error(`ESI ${response.status} for ${url}`);
  return response.json() as Promise<T>;
}

function toOrder(row: any) {
  return {
    locationId: Number(row.location_id),
    regionId: Number(row.region_id),
    price: Number(row.price),
    volumeRemain: Number(row.volume_remain),
    isBuyOrder: Boolean(row.is_buy_order),
    issued: row.issued,
    duration: row.duration,
    orderId: row.order_id
  };
}

function recentHistoryVolume(rows: any[], days: number): number {
  const cutoff = Date.now() - days * 86400000;
  return rows.reduce((sum, row) => new Date(`${row.date}T00:00:00Z`).getTime() >= cutoff ? sum + Number(row.volume || 0) : sum, 0);
}

function upsertOpportunity(store: StoreShape, opportunity: Opportunity) {
  const index = store.opportunities.findIndex((row) => row.typeId === opportunity.typeId);
  if (index >= 0) store.opportunities[index] = opportunity;
  else store.opportunities.push(opportunity);
}

function settingValue(store: StoreShape, key: string, fallback: string): string {
  return store.settings.find((setting) => setting.key === key)?.value ?? fallback;
}

function mustFindProduct(store: StoreShape, typeId: number): Product {
  const product = store.products.find((row) => row.typeId === typeId);
  if (!product) throw new Error(`Unknown product ${typeId}`);
  return product;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
