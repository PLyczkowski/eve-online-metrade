import type {
  AnalyzeInput,
  HubName,
  MarketConfig,
  MarketHistoryRow,
  Opportunity,
  Order,
  Product
} from "../types";

interface HubData {
  name: HubName;
  stationId: number;
  regionId: number;
  sells: Order[];
  lowestSell: number;
  referenceSell: number;
  availableAtLowest: number;
  orderCount: number;
  buyOrderCount: number;
  buyOrderUnits: number;
  volume: number;
  historyReference: number;
}

export const defaultMarketConfig: MarketConfig = {
  jitaStationId: 60003760,
  amarrStationId: 60008494,
  theForgeRegionId: 10000002,
  domainRegionId: 10000043,
  minimumSpread: 0.2,
  minimumEstimatedProfit: 500000,
  minimumSourceVolume: 1,
  minimumDestinationVolume: 1,
  historyDays: 30,
  includeWeakRows: true,
  sellReferenceMinimumUnits: 5,
  sellReferenceMinimumIskDepth: 25000000,
  shipCargoCapacityM3: 7900,
  suggestedBuyDestinationVolumePercent: 0.3,
  emptyDestinationMaxVolumePercent: 0.15,
  scoreTargetProfit: 100000000,
  scoreProfitWeight: 50,
  scoreSellThroughWeight: 40,
  scoreCargoWeight: 10
};

export function recentVolume(history: MarketHistoryRow[], days: number, now = new Date()): number {
  const cutoff = new Date(now);
  cutoff.setDate(cutoff.getDate() - days);
  return history.reduce((sum, row) => {
    const date = new Date(`${row.date}T00:00:00Z`);
    return date >= cutoff ? sum + Number(row.volume || 0) : sum;
  }, 0);
}

export function analyzeOpportunity(input: AnalyzeInput): Opportunity {
  const { product, forgeOrders, domainOrders, forgeVolume, domainVolume, forgeHistoryAverage = 0, domainHistoryAverage = 0, refreshedAt, config } = input;
  const jita = hubSellData("Jita", config.jitaStationId, config.theForgeRegionId, forgeOrders, forgeVolume, forgeHistoryAverage, config);
  const amarr = hubSellData("Amarr", config.amarrStationId, config.domainRegionId, domainOrders, domainVolume, domainHistoryAverage, config);

  if (!jita.lowestSell && !amarr.lowestSell) {
    return emptyMarketRow("NO SELL ORDERS", product.typeId, product.name, jita.volume, amarr.volume, refreshedAt, "No sell orders at either hub station", product.notes);
  }
  if (!jita.lowestSell) {
    const row = emptyDestinationOpportunity(amarr, jita, product, refreshedAt, config);
    if (row) return row;
    return emptyMarketRow("NO JITA SELL", product.typeId, product.name, jita.volume, amarr.volume, refreshedAt, "No Jita sell orders at hub station", product.notes);
  }
  if (!amarr.lowestSell) {
    const row = emptyDestinationOpportunity(jita, amarr, product, refreshedAt, config);
    if (row) return row;
    return emptyMarketRow("NO AMARR SELL", product.typeId, product.name, jita.volume, amarr.volume, refreshedAt, "No Amarr sell orders at hub station", product.notes);
  }

  const jitaToAmarrProfit = amarr.referenceSell - jita.lowestSell;
  const amarrToJitaProfit = jita.referenceSell - amarr.lowestSell;
  const buyHub = jitaToAmarrProfit >= amarrToJitaProfit ? jita : amarr;
  const sellHub = buyHub === jita ? amarr : jita;
  const buyPrice = buyHub.lowestSell;
  const sellReference = sellHub.referenceSell;
  const profitPerUnit = sellReference - buyPrice;
  const spread = buyPrice > 0 ? profitPerUnit / buyPrice : 0;
  const sourceAvailable = buyHub.availableAtLowest;
  const cargoUnits = cargoUnitCapacity(config.shipCargoCapacityM3, product.volumeM3 ?? null);
  const destinationVolumeUnits = Math.floor(sellHub.volume * Math.max(0, Math.min(1, config.suggestedBuyDestinationVolumePercent)));
  const suggestedBuyQuantity = suggestedBuyUnits(sourceAvailable, cargoUnits, destinationVolumeUnits);
  const estimatedProfit = Math.max(0, suggestedBuyQuantity * profitPerUnit);
  const cargoUsedPercent = cargoUsedFraction(config.shipCargoCapacityM3, product.volumeM3 ?? null, suggestedBuyQuantity);
  const score = opportunityScore(estimatedProfit, cargoUsedPercent, suggestedBuyQuantity, sellHub.volume, sellHub.orderCount, config);

  let status: Opportunity["status"] = "GOOD";
  let scriptNotes = "Both prices are sell orders; direction is chosen from lower sell price to higher sell price.";
  if (profitPerUnit <= 0) {
    status = "NO SPREAD";
    scriptNotes = "Depth-adjusted sell reference is equal or inverted.";
  } else if (spread < config.minimumSpread) {
    status = "LOW SPREAD";
    scriptNotes = "Below minimum spread.";
  } else if (estimatedProfit < config.minimumEstimatedProfit) {
    status = "LOW PROFIT";
    scriptNotes = "Below minimum estimated profit.";
  } else if (buyHub.volume < config.minimumSourceVolume || sellHub.volume < config.minimumDestinationVolume) {
    status = "LOW TRAFFIC";
    scriptNotes = "Below recent regional volume threshold.";
  }

  return {
    status,
    direction: `${buyHub.name} -> ${sellHub.name}`,
    typeId: product.typeId,
    itemName: product.name,
    buyHub: buyHub.name,
    sellHub: sellHub.name,
    buyPrice,
    sellReference,
    destinationLowestSell: sellHub.lowestSell,
    profitPerUnit,
    spread,
    sourceAvailable,
    estimatedProfit,
    score,
    cargoUsedPercent,
    suggestedBuyQuantity,
    destinationOrderCount: sellHub.orderCount,
    myDestinationSellPriceMin: null,
    myDestinationSellPriceMax: null,
    myDestinationSellQuantity: null,
    myDestinationSellOrderCount: null,
    buyRegionVolume: buyHub.volume,
    sellRegionVolume: sellHub.volume,
    lastRefresh: refreshedAt,
    lastRefreshMinutes: 0,
    notes: product.notes,
    scriptNotes
  };
}

function hubSellData(name: HubName, stationId: number, regionId: number, orders: Order[], volume: number, historyReference: number, config: MarketConfig): HubData {
  const sells = orders
    .filter((order) => !order.isBuyOrder && Number(order.locationId) === Number(stationId))
    .sort((left, right) => left.price - right.price);
  const buys = orders.filter((order) => order.isBuyOrder);
  const lowestSell = sells.length ? Number(sells[0].price) : 0;
  const depth = referenceSellPrice(sells, config.sellReferenceMinimumUnits, config.sellReferenceMinimumIskDepth);
  return {
    name,
    stationId,
    regionId,
    sells,
    lowestSell,
    referenceSell: depth,
    availableAtLowest: sells.filter((order) => Number(order.price) <= lowestSell).reduce((sum, order) => sum + Number(order.volumeRemain || 0), 0),
    orderCount: sells.length,
    buyOrderCount: buys.length,
    buyOrderUnits: buys.reduce((sum, order) => sum + Number(order.volumeRemain || 0), 0),
    volume,
    historyReference
  };
}

function emptyDestinationOpportunity(sourceHub: HubData, destinationHub: HubData, product: Product, refreshedAt: string, config: MarketConfig): Opportunity | null {
  if (!sourceHub.lowestSell || destinationHub.lowestSell || destinationHub.historyReference <= 0 || destinationHub.volume < config.minimumDestinationVolume) return null;
  const buyPrice = sourceHub.lowestSell;
  const sellReference = destinationHub.historyReference;
  const profitPerUnit = sellReference - buyPrice;
  if (profitPerUnit <= 0) return null;
  const sourceAvailable = sourceHub.availableAtLowest;
  const cargoUnits = cargoUnitCapacity(config.shipCargoCapacityM3, product.volumeM3 ?? null);
  const destinationVolumeUnits = Math.floor(destinationHub.volume * Math.max(0, Math.min(1, config.emptyDestinationMaxVolumePercent)));
  const suggestedBuyQuantity = suggestedBuyUnits(sourceAvailable, cargoUnits, destinationVolumeUnits);
  const estimatedProfit = Math.max(0, suggestedBuyQuantity * profitPerUnit);
  const cargoUsedPercent = cargoUsedFraction(config.shipCargoCapacityM3, product.volumeM3 ?? null, suggestedBuyQuantity);
  const score = opportunityScore(estimatedProfit, cargoUsedPercent, suggestedBuyQuantity, destinationHub.volume, 0, config);
  const status: Opportunity["status"] = estimatedProfit < config.minimumEstimatedProfit ? "LOW PROFIT" : "EMPTY DEST";
  return {
    status,
    direction: `${sourceHub.name} -> ${destinationHub.name}`,
    typeId: product.typeId,
    itemName: product.name,
    buyHub: sourceHub.name,
    sellHub: destinationHub.name,
    buyPrice,
    sellReference,
    destinationLowestSell: null,
    profitPerUnit,
    spread: buyPrice > 0 ? profitPerUnit / buyPrice : 0,
    sourceAvailable,
    estimatedProfit,
    score,
    cargoUsedPercent,
    suggestedBuyQuantity,
    destinationOrderCount: 0,
    myDestinationSellPriceMin: null,
    myDestinationSellPriceMax: null,
    myDestinationSellQuantity: null,
    myDestinationSellOrderCount: null,
    buyRegionVolume: sourceHub.volume,
    sellRegionVolume: destinationHub.volume,
    lastRefresh: refreshedAt,
    lastRefreshMinutes: 0,
    notes: product.notes,
    scriptNotes: `No sell orders at destination station; sell reference uses destination regional 30-day average. Destination buy orders: ${destinationHub.buyOrderCount}, buy units: ${Math.round(destinationHub.buyOrderUnits)}. Suggested buy is capped at 15% of destination 30-day volume.`
  };
}

function referenceSellPrice(orders: Order[], minimumUnits: number, minimumIskDepth: number): number {
  if (!orders.length) return 0;
  let cumulativeUnits = 0;
  let cumulativeIsk = 0;
  for (const order of orders) {
    const units = Math.max(0, Number(order.volumeRemain || 0));
    cumulativeUnits += units;
    cumulativeIsk += units * Math.max(0, Number(order.price || 0));
    if (cumulativeUnits >= minimumUnits || (minimumIskDepth > 0 && cumulativeIsk >= minimumIskDepth)) {
      return Number(order.price);
    }
  }
  return Number(orders[orders.length - 1].price);
}

function cargoUnitCapacity(cargoM3: number, volumeM3: number | null): number | null {
  if (!volumeM3 || volumeM3 <= 0 || cargoM3 <= 0) return null;
  return Math.max(0, Math.floor(cargoM3 / volumeM3));
}

function suggestedBuyUnits(sourceAvailable: number, cargoUnits: number | null, destinationVolumeUnits: number): number {
  const cap = cargoUnits === null ? sourceAvailable : Math.min(sourceAvailable, cargoUnits);
  return Math.max(0, Math.floor(Math.min(cap, destinationVolumeUnits)));
}

function cargoUsedFraction(cargoM3: number, volumeM3: number | null, estimatedUnits: number): number | null {
  if (!volumeM3 || volumeM3 <= 0 || cargoM3 <= 0) return null;
  return Math.min(1, Math.max(0, (estimatedUnits * volumeM3) / cargoM3));
}

function opportunityScore(estimatedProfit: number | null, cargoUsedPercent: number | null, suggestedBuyQuantity: number | null, sellRegionVolume: number | null, destinationOrderCount: number | null, config: MarketConfig): number | null {
  if (estimatedProfit === null) return null;
  if (estimatedProfit <= 0) return 0;
  const profitWeight = Math.max(0, config.scoreProfitWeight);
  const sellThroughWeight = Math.max(0, config.scoreSellThroughWeight);
  const cargoWeight = Math.max(0, config.scoreCargoWeight);
  const destinationOrderWeight = 60;
  const totalWeight = Math.max(1, profitWeight + sellThroughWeight + cargoWeight + destinationOrderWeight);
  const profitScore = Math.min(1, Math.max(0, estimatedProfit / Math.max(1, config.scoreTargetProfit)));
  const cargoScore = cargoUsedPercent === null ? 0.5 : Math.min(1, Math.max(0, 1 - cargoUsedPercent));
  const velocityScore = suggestedBuyQuantity !== null && sellRegionVolume !== null && suggestedBuyQuantity > 0 && sellRegionVolume > 0
    ? Math.min(1, Math.max(0, 1 - Math.min(1, suggestedBuyQuantity / sellRegionVolume)))
    : 0;
  const orderScore = destinationOrderScore(destinationOrderCount);
  return ((profitScore * profitWeight + velocityScore * sellThroughWeight + cargoScore * cargoWeight + orderScore * destinationOrderWeight) / totalWeight) * 100;
}

function destinationOrderScore(count: number | null): number {
  if (count == null) return 0.5;
  return Math.min(1, Math.max(0, 1 - (Math.max(1, count) - 1) / 24));
}

function emptyMarketRow(
  status: Opportunity["status"],
  typeId: number,
  itemName: string,
  buyRegionVolume: number,
  sellRegionVolume: number,
  refreshedAt: string,
  scriptNotes: string,
  notes: string
): Opportunity {
  return {
    status,
    direction: "",
    typeId,
    itemName,
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
    destinationOrderCount: null,
    myDestinationSellPriceMin: null,
    myDestinationSellPriceMax: null,
    myDestinationSellQuantity: null,
    myDestinationSellOrderCount: null,
    buyRegionVolume,
    sellRegionVolume,
    lastRefresh: refreshedAt,
    lastRefreshMinutes: 0,
    notes,
    scriptNotes
  };
}

export function shouldSkipLowTargetVolume(opportunity: Opportunity | undefined, minimumVolume: number): boolean {
  if (!opportunity || !minimumVolume || minimumVolume <= 0) return false;
  return opportunity.sellRegionVolume !== null && opportunity.sellRegionVolume < minimumVolume;
}
