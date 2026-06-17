export type Status =
  | "PENDING"
  | "GOOD"
  | "LOW SPREAD"
  | "LOW PROFIT"
  | "LOW TRAFFIC"
  | "NO SPREAD"
  | "NO SELL ORDERS"
  | "NO JITA SELL"
  | "NO AMARR SELL"
  | "ERROR";

export type HubName = "Jita" | "Amarr";

export interface Product {
  typeId: number;
  name: string;
  enabled: boolean;
  notes: string;
  volumeM3?: number | null;
}

export interface Setting {
  key: string;
  value: string;
  notes: string;
}

export interface Opportunity {
  status: Status;
  direction: string;
  typeId: number;
  itemName: string;
  buyHub: string;
  sellHub: string;
  buyPrice: number | null;
  sellReference: number | null;
  profitPerUnit: number | null;
  spread: number | null;
  sourceAvailable: number | null;
  estimatedProfit: number | null;
  cargoUsedPercent: number | null;
  buyRegionVolume: number | null;
  sellRegionVolume: number | null;
  lastRefresh: string | null;
  lastRefreshMinutes: number | null;
  notes: string;
  scriptNotes: string;
}

export interface RefreshRun {
  refreshTime: string;
  itemsScanned: number;
  opportunitiesWritten: number;
  apiCalls: number;
  errors: string;
  skipped: string;
  durationSeconds: number;
}

export interface RefreshJob {
  status: "idle" | "running" | "done" | "failed";
  kind: string;
  currentItem: string;
  scannedCount: number;
  totalCount: number;
  apiCalls: number;
  lastError: string;
  queuedCount: number;
  startedAt: string;
  finishedAt: string;
}

export interface DiscoverySummary {
  knownItems: number;
  marketRows: number;
  candidates: number;
  products: number;
  enabledProducts: number;
  lastDiscovery: string;
}

export interface DiscoveryRun {
  runTime: string;
  itemTypesImported: number;
  marketRowsImported: number;
  candidatesFound: number;
  productsEnabled: number;
  errors: string;
  durationSeconds: number;
}

export interface ApiLimitStatus {
  lastResponseAt: string;
  lastStatus: number;
  errorLimitRemain: number | null;
  errorLimitReset: number | null;
  retryAfter: number | null;
  rateLimitLimit: string;
  rateLimitRemaining: number | null;
  rateLimitUsed: number | null;
  rateLimited: boolean;
  lastUrl: string;
}

export interface Order {
  locationId: number;
  regionId: number;
  price: number;
  volumeRemain: number;
  isBuyOrder: boolean;
  issued?: string;
  duration?: number;
  orderId?: number;
}

export interface MarketHistoryRow {
  date: string;
  volume: number;
}

export interface MarketConfig {
  jitaStationId: number;
  amarrStationId: number;
  theForgeRegionId: number;
  domainRegionId: number;
  minimumSpread: number;
  minimumEstimatedProfit: number;
  minimumSourceVolume: number;
  minimumDestinationVolume: number;
  historyDays: number;
  includeWeakRows: boolean;
  sellReferenceMinimumUnits: number;
  sellReferenceMinimumIskDepth: number;
  shipCargoCapacityM3: number;
}

export interface AnalyzeInput {
  product: Product;
  forgeOrders: Order[];
  domainOrders: Order[];
  forgeVolume: number;
  domainVolume: number;
  refreshedAt: string;
  config: MarketConfig;
}

export interface Filters {
  search: string;
  status: string;
  direction: string;
}
