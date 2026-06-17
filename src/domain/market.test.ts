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
      forgeVolume: 100,
      domainVolume: 100
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
      forgeVolume: 100,
      domainVolume: 100
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
