import type {
  AnalyzeInput,
  HubName,
  MarketConfig,
  MarketHistoryRow,
  Opportunity,
  Order
} from "../types";

interface HubData {
  name: HubName;
  stationId: number;
  regionId: number;
  sells: Order[];
  lowestSell: number;
  volume: number;
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
  includeWeakRows: true
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
  const { product, forgeOrders, domainOrders, forgeVolume, domainVolume, refreshedAt, config } = input;
  const jita = hubSellData("Jita", config.jitaStationId, config.theForgeRegionId, forgeOrders, forgeVolume);
  const amarr = hubSellData("Amarr", config.amarrStationId, config.domainRegionId, domainOrders, domainVolume);

  if (!jita.lowestSell && !amarr.lowestSell) {
    return emptyMarketRow("NO SELL ORDERS", product.typeId, product.name, jita.volume, amarr.volume, refreshedAt, "No sell orders at either hub station", product.notes);
  }
  if (!jita.lowestSell) {
    return emptyMarketRow("NO JITA SELL", product.typeId, product.name, jita.volume, amarr.volume, refreshedAt, "No Jita sell orders at hub station", product.notes);
  }
  if (!amarr.lowestSell) {
    return emptyMarketRow("NO AMARR SELL", product.typeId, product.name, jita.volume, amarr.volume, refreshedAt, "No Amarr sell orders at hub station", product.notes);
  }

  const buyHub = jita.lowestSell <= amarr.lowestSell ? jita : amarr;
  const sellHub = buyHub === jita ? amarr : jita;
  const buyPrice = buyHub.lowestSell;
  const sellReference = sellHub.lowestSell;
  const profitPerUnit = sellReference - buyPrice;
  const spread = buyPrice > 0 ? profitPerUnit / buyPrice : 0;
  const sourceAvailable = buyHub.sells
    .filter((order) => Number(order.price) <= buyPrice)
    .reduce((sum, order) => sum + Number(order.volumeRemain || 0), 0);
  const estimatedProfit = Math.max(0, sourceAvailable * profitPerUnit);

  let status: Opportunity["status"] = "GOOD";
  let scriptNotes = "Both prices are sell orders; direction is chosen from lower sell price to higher sell price.";
  if (profitPerUnit <= 0) {
    status = "NO SPREAD";
    scriptNotes = "Lowest sell prices are equal or inverted.";
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
    profitPerUnit,
    spread,
    sourceAvailable,
    estimatedProfit,
    buyRegionVolume: buyHub.volume,
    sellRegionVolume: sellHub.volume,
    lastRefresh: refreshedAt,
    lastRefreshMinutes: 0,
    notes: product.notes,
    scriptNotes
  };
}

function hubSellData(name: HubName, stationId: number, regionId: number, orders: Order[], volume: number): HubData {
  const sells = orders
    .filter((order) => !order.isBuyOrder && Number(order.locationId) === Number(stationId))
    .sort((left, right) => left.price - right.price);
  return {
    name,
    stationId,
    regionId,
    sells,
    lowestSell: sells.length ? Number(sells[0].price) : 0,
    volume
  };
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
    profitPerUnit: null,
    spread: null,
    sourceAvailable: null,
    estimatedProfit: null,
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
