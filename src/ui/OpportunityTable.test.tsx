import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { OpportunityTable } from "./OpportunityTable";
import type { Opportunity } from "../types";

const row: Opportunity = {
  status: "GOOD",
  direction: "Jita -> Amarr",
  typeId: 33180,
  itemName: "Scan Rangefinding Array I",
  buyHub: "Jita",
  sellHub: "Amarr",
  buyPrice: 113900,
  sellReference: 157900,
  profitPerUnit: 44000,
  spread: 0.3863,
  sourceAvailable: 93,
  estimatedProfit: 4092000,
  buyRegionVolume: 42069,
  sellRegionVolume: 5526,
  lastRefresh: "2026-06-17T12:00:00Z",
  lastRefreshMinutes: 12,
  notes: "",
  scriptNotes: ""
};

describe("OpportunityTable", () => {
  it("shows right-click update action for a row", async () => {
    const onRefreshRow = vi.fn().mockResolvedValue(undefined);
    render(
      <OpportunityTable
        rows={[row]}
        onRefreshRow={onRefreshRow}
        onEditNotes={vi.fn()}
        onDisableProduct={vi.fn()}
      />
    );

    fireEvent.contextMenu(screen.getByText("Scan Rangefinding Array I"));
    fireEvent.click(screen.getByText("Update data"));

    await waitFor(() => expect(onRefreshRow).toHaveBeenCalledWith(33180));
  });
});
