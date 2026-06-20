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

export interface TradeHub {
  id: number;
  name: string;
  regionId: number;
  stationId: number;
  enabled: boolean;
  priority: number;
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
  destinationLowestSell: number | null;
  profitPerUnit: number | null;
  spread: number | null;
  sourceAvailable: number | null;
  estimatedProfit: number | null;
  score: number | null;
  cargoUsedPercent: number | null;
  suggestedBuyQuantity: number | null;
  destinationOrderCount: number | null;
  myDestinationSellPriceMin: number | null;
  myDestinationSellPriceMax: number | null;
  myDestinationSellQuantity: number | null;
  myDestinationSellOrderCount: number | null;
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
  lastProgressAt: string;
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

export interface AuthCharacter {
  characterId: number;
  characterName: string;
  scopes: string;
  expiresAt: string;
  updatedAt: string;
}

export interface AuthEvent {
  happenedAt: string;
  status: string;
  message: string;
}

export interface CharacterOrder {
  characterId: number;
  orderId: number;
  typeId: number;
  regionId: number;
  locationId: number;
  isBuyOrder: boolean;
  price: number;
  volumeRemain: number;
  volumeTotal: number;
  issued: string;
  duration: number;
  range: string;
  state: string;
  refreshedAt: string;
}

export interface WalletTransaction {
  characterId: number;
  transactionId: number;
  transactionDate: string;
  typeId: number;
  itemName: string;
  locationId: number;
  stationName: string;
  quantity: number;
  unitPrice: number;
  totalPrice: number;
  isBuy: boolean;
  clientId: number;
  matchedOrderId: number | null;
}

export interface SaleNotification {
  id: number;
  characterId: number;
  transactionId: number;
  happenedAt: string;
  itemName: string;
  quantity: number;
  unitPrice: number;
  totalPrice: number;
  seen: boolean;
}

export interface OurOrder {
  characterId: number;
  characterName: string;
  orderId: number;
  typeId: number;
  itemName: string;
  regionId: number;
  locationId: number;
  stationName: string;
  price: number;
  volumeRemain: number;
  volumeTotal: number;
  issued: string;
  expiresAt: string;
  refreshedAt: string;
  lowestCompetingPrice: number | null;
  isUndercut: boolean;
  suggestedUpdatePrice: number | null;
  estimatedUpdateFee: number | null;
  boughtUnitPrice: number | null;
  boughtQuantityMatched: number | null;
  expectedProfitPerUnit: number | null;
  expectedProfitRemaining: number | null;
  manualCost: boolean;
}

export interface AccountRefreshResult {
  characterId: number;
  orders: number;
  transactions: number;
  newSaleNotifications: number;
  apiCalls: number;
  message: string;
}

export interface AccountFilters {
  search: string;
  characterId: number | null;
  station: string;
  undercutOnly: boolean;
  unknownCostOnly: boolean;
  side: string;
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
  suggestedBuyDestinationVolumePercent: number;
  scoreTargetProfit: number;
  scoreProfitWeight: number;
  scoreSellThroughWeight: number;
  scoreCargoWeight: number;
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
