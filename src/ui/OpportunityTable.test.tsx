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
  destinationLowestSell: 157900,
  profitPerUnit: 44000,
  spread: 0.3863,
  sourceAvailable: 93,
  estimatedProfit: 4092000,
  score: 42,
  cargoUsedPercent: 0.42,
  suggestedBuyQuantity: 93,
  destinationOrderCount: 3,
  myDestinationSellPriceMin: null,
  myDestinationSellPriceMax: null,
  myDestinationSellQuantity: null,
  myDestinationSellOrderCount: null,
  buyRegionVolume: 42069,
  sellRegionVolume: 5526,
  lastRefresh: "2026-06-17T12:00:00Z",
  lastRefreshMinutes: 12,
  notes: "",
  scriptNotes: ""
};

const secondRow: Opportunity = {
  ...row,
  typeId: 33181,
  itemName: "Scan Pinpointing Array I"
};

const thirdRow: Opportunity = {
  ...row,
  typeId: 33182,
  itemName: "Scan Acquisition Array I"
};

describe("OpportunityTable", () => {
  it("shows right-click update action for a row", async () => {
    const onRefreshRow = vi.fn().mockResolvedValue(undefined);
    render(
      <OpportunityTable
        rows={[row]}
        onRefreshRow={onRefreshRow}
        onRefreshRows={vi.fn()}
        onEditNotes={vi.fn()}
        onDisableProduct={vi.fn()}
      />
    );

    fireEvent.contextMenu(screen.getByText("Scan Rangefinding Array I"));
    fireEvent.click(screen.getByText("Update data"));

    await waitFor(() => expect(onRefreshRow).toHaveBeenCalledWith(33180));
  });

  it("updates selected rows from the context menu", async () => {
    const onRefreshRows = vi.fn().mockResolvedValue(undefined);
    const { container } = render(
      <OpportunityTable
        rows={[row, secondRow, thirdRow]}
        onRefreshRow={vi.fn()}
        onRefreshRows={onRefreshRows}
        onEditNotes={vi.fn()}
        onDisableProduct={vi.fn()}
      />
    );
    const tableRows = container.querySelectorAll("tbody tr");

    fireEvent.click(tableRows[0]);
    fireEvent.click(tableRows[1], { ctrlKey: true });
    fireEvent.contextMenu(tableRows[1]);
    fireEvent.click(screen.getByText(/Update Selected/));

    await waitFor(() => expect(onRefreshRows).toHaveBeenCalledWith([33180, 33181]));
  });

  it("clears selected rows with Escape", () => {
    const { container } = render(
      <OpportunityTable
        rows={[row, secondRow]}
        onRefreshRow={vi.fn()}
        onRefreshRows={vi.fn()}
        onEditNotes={vi.fn()}
        onDisableProduct={vi.fn()}
      />
    );
    const shell = container.querySelector(".table-shell") as HTMLElement;
    const tableRows = container.querySelectorAll("tbody tr");

    fireEvent.click(tableRows[0]);
    expect(tableRows[0].className).toContain("is-selected");
    fireEvent.keyDown(shell, { key: "Escape" });
    expect(tableRows[0].className).not.toContain("is-selected");
  });

  it("shows destination orders", () => {
    render(
      <OpportunityTable
        rows={[row]}
        onRefreshRow={vi.fn()}
        onRefreshRows={vi.fn()}
        onEditNotes={vi.fn()}
        onDisableProduct={vi.fn()}
      />
    );

    expect(screen.getAllByText("Dest Orders").length).toBeGreaterThan(0);
    expect(screen.getAllByText("3").length).toBeGreaterThan(0);
  });
});
