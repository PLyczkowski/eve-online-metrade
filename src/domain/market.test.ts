import { describe, expect, it } from "vitest";
import { analyzeOpportunity, defaultMarketConfig, shouldSkipLowTargetVolume } from "./market";
import type { Product, Order, Opportunity } from "../types";

const product: Product = { typeId: 1, name: "Test Item", enabled: true, notes: "" };

function order(locationId: number, price: number, volumeRemain = 10): Order {
  return {
    locationId,
    regionId: 10000002,
    price,
    volumeRemain,
    isBuyOrder: false
  };
}

describe("analyzeOpportunity", () => {
  it("chooses Jita to Amarr when Jita has the lower sell price", () => {
    const row = analyzeOpportunity({
      product,
      config: defaultMarketConfig,
      refreshedAt: "2026-06-17T12:00:00Z",
      forgeOrders: [order(defaultMarketConfig.jitaStationId, 100, 10000)],
      domainOrders: [order(defaultMarketConfig.amarrStationId, 160, 10000)],
      forgeVolume: 100000,
      domainVolume: 100000
    });

    expect(row.status).toBe("GOOD");
    expect(row.direction).toBe("Jita -> Amarr");
    expect(row.spread).toBeCloseTo(0.6);
    expect(row.estimatedProfit).toBe(600000);
  });

  it("chooses Amarr to Jita when Amarr has the lower sell price", () => {
    const row = analyzeOpportunity({
      product,
      config: defaultMarketConfig,
      refreshedAt: "2026-06-17T12:00:00Z",
      forgeOrders: [order(defaultMarketConfig.jitaStationId, 220, 10000)],
      domainOrders: [order(defaultMarketConfig.amarrStationId, 100, 10000)],
      forgeVolume: 100000,
      domainVolume: 100000
    });

    expect(row.status).toBe("GOOD");
    expect(row.direction).toBe("Amarr -> Jita");
    expect(row.profitPerUnit).toBe(120);
  });

  it("marks missing Amarr sell orders", () => {
    const row = analyzeOpportunity({
      product,
      config: defaultMarketConfig,
      refreshedAt: "2026-06-17T12:00:00Z",
      forgeOrders: [order(defaultMarketConfig.jitaStationId, 100)],
      domainOrders: [],
      forgeVolume: 100,
      domainVolume: 100
    });

    expect(row.status).toBe("NO AMARR SELL");
  });

  it("creates empty destination opportunities from regional history demand", () => {
    const row = analyzeOpportunity({
      product: { ...product, volumeM3: 1 },
      config: { ...defaultMarketConfig, minimumEstimatedProfit: 1, emptyDestinationMaxVolumePercent: 0.15 },
      refreshedAt: "2026-06-17T12:00:00Z",
      forgeOrders: [order(defaultMarketConfig.jitaStationId, 100, 1000)],
      domainOrders: [order(123, 120, 500)],
      forgeVolume: 1000,
      domainVolume: 100,
      domainHistoryAverage: 200
    });

    expect(row.status).toBe("EMPTY DEST");
    expect(row.direction).toBe("Jita -> Amarr");
    expect(row.sellReference).toBe(200);
    expect(row.destinationLowestSell).toBeNull();
    expect(row.destinationOrderCount).toBe(0);
    expect(row.suggestedBuyQuantity).toBe(15);
    expect(row.estimatedProfit).toBe(1500);
  });

  it("marks low spread before low profit", () => {
    const row = analyzeOpportunity({
      product,
      config: defaultMarketConfig,
      refreshedAt: "2026-06-17T12:00:00Z",
      forgeOrders: [order(defaultMarketConfig.jitaStationId, 100, 1)],
      domainOrders: [order(defaultMarketConfig.amarrStationId, 105, 1)],
      forgeVolume: 100,
      domainVolume: 100
    });

    expect(row.status).toBe("LOW SPREAD");
  });

  it("marks low traffic after spread and profit pass", () => {
    const row = analyzeOpportunity({
      product,
      config: { ...defaultMarketConfig, minimumEstimatedProfit: 1 },
      refreshedAt: "2026-06-17T12:00:00Z",
      forgeOrders: [order(defaultMarketConfig.jitaStationId, 100, 10)],
      domainOrders: [order(defaultMarketConfig.amarrStationId, 200, 10)],
      forgeVolume: 0,
      domainVolume: 100
    });

    expect(row.status).toBe("LOW TRAFFIC");
  });

  it("uses a depth-based sell reference to avoid one-off sell order skew", () => {
    const row = analyzeOpportunity({
      product,
      config: { ...defaultMarketConfig, minimumEstimatedProfit: 1, sellReferenceMinimumUnits: 5, sellReferenceMinimumIskDepth: 1000000000000 },
      refreshedAt: "2026-06-17T12:00:00Z",
      forgeOrders: [order(defaultMarketConfig.jitaStationId, 100, 100)],
      domainOrders: [
        order(defaultMarketConfig.amarrStationId, 101, 1),
        order(defaultMarketConfig.amarrStationId, 150, 100)
      ],
      forgeVolume: 100,
      domainVolume: 100
    });

    expect(row.direction).toBe("Jita -> Amarr");
    expect(row.sellReference).toBe(150);
    expect(row.profitPerUnit).toBe(50);
  });

  it("caps estimated profit by ship cargo capacity", () => {
    const row = analyzeOpportunity({
      product: { ...product, volumeM3: 10 },
      config: { ...defaultMarketConfig, minimumEstimatedProfit: 1, shipCargoCapacityM3: 50 },
      refreshedAt: "2026-06-17T12:00:00Z",
      forgeOrders: [order(defaultMarketConfig.jitaStationId, 100, 100)],
      domainOrders: [order(defaultMarketConfig.amarrStationId, 200, 100)],
      forgeVolume: 100,
      domainVolume: 100
    });

    expect(row.sourceAvailable).toBe(100);
    expect(row.estimatedProfit).toBe(500);
    expect(row.cargoUsedPercent).toBe(1);
    expect(row.suggestedBuyQuantity).toBe(5);
  });

  it("caps suggested buy by destination 30-day volume share", () => {
    const row = analyzeOpportunity({
      product: { ...product, volumeM3: 1 },
      config: { ...defaultMarketConfig, minimumEstimatedProfit: 1, suggestedBuyDestinationVolumePercent: 0.3 },
      refreshedAt: "2026-06-17T12:00:00Z",
      forgeOrders: [order(defaultMarketConfig.jitaStationId, 100, 1000)],
      domainOrders: [order(defaultMarketConfig.amarrStationId, 200, 1000)],
      forgeVolume: 1000,
      domainVolume: 20
    });

    expect(row.suggestedBuyQuantity).toBe(6);
    expect(row.estimatedProfit).toBe(600);
  });

  it("counts destination station sell orders", () => {
    const row = analyzeOpportunity({
      product,
      config: { ...defaultMarketConfig, minimumEstimatedProfit: 1 },
      refreshedAt: "2026-06-17T12:00:00Z",
      forgeOrders: [order(defaultMarketConfig.jitaStationId, 100, 1000)],
      domainOrders: [
        order(defaultMarketConfig.amarrStationId, 200, 1000),
        order(defaultMarketConfig.amarrStationId, 210, 1000),
        order(defaultMarketConfig.amarrStationId, 220, 1000),
        order(123, 150, 1000)
      ],
      forgeVolume: 1000,
      domainVolume: 1000
    });

    expect(row.direction).toBe("Jita -> Amarr");
    expect(row.destinationOrderCount).toBe(3);
  });

  it("scores lower destination order counts higher", () => {
    const lowCompetition = analyzeOpportunity({
      product,
      config: { ...defaultMarketConfig, minimumEstimatedProfit: 1 },
      refreshedAt: "2026-06-17T12:00:00Z",
      forgeOrders: [order(defaultMarketConfig.jitaStationId, 100, 1000)],
      domainOrders: [order(defaultMarketConfig.amarrStationId, 200, 1000)],
      forgeVolume: 1000,
      domainVolume: 1000
    });
    const highCompetition = analyzeOpportunity({
      product,
      config: { ...defaultMarketConfig, minimumEstimatedProfit: 1 },
      refreshedAt: "2026-06-17T12:00:00Z",
      forgeOrders: [order(defaultMarketConfig.jitaStationId, 100, 1000)],
      domainOrders: Array.from({ length: 30 }, (_, index) => order(defaultMarketConfig.amarrStationId, 200 + index, 1000)),
      forgeVolume: 1000,
      domainVolume: 1000
    });

    expect(lowCompetition.destinationOrderCount).toBe(1);
    expect(highCompetition.destinationOrderCount).toBe(30);
    expect(lowCompetition.score ?? 0).toBeGreaterThan(highCompetition.score ?? 0);
  });
});

describe("shouldSkipLowTargetVolume", () => {
  it("skips rows with known target volume below threshold", () => {
    const row = { sellRegionVolume: 12 } as Opportunity;
    expect(shouldSkipLowTargetVolume(row, 50)).toBe(true);
  });

  it("does not skip unknown rows", () => {
    expect(shouldSkipLowTargetVolume(undefined, 50)).toBe(false);
  });
});
