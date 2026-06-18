import { CSSProperties, Dispatch, MouseEvent, SetStateAction, useMemo, useRef, useState } from "react";
import {
  ColumnDef,
  ColumnSizingState,
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  SortingState,
  useReactTable
} from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { Opportunity } from "../types";
import { formatIsk, formatPercent } from "../domain/format";

const columnSizingStorageKey = "eve-metrade-column-sizing-v1";

interface Props {
  rows: Opportunity[];
  onRefreshRow: (typeId: number) => Promise<void>;
  onRefreshRows: (typeIds: number[]) => Promise<void>;
  onEditNotes: (typeId: number, current: string) => Promise<void>;
  onDisableProduct: (typeId: number) => Promise<void>;
}

export function OpportunityTable({ rows, onRefreshRow, onRefreshRows, onEditNotes, onDisableProduct }: Props) {
  const [sorting, setSorting] = useState<SortingState>([{ id: "estimatedProfit", desc: true }]);
  const [columnSizing, setColumnSizing] = useState<ColumnSizingState>(() => readSavedColumnSizing());
  const [menu, setMenu] = useState<{ x: number; y: number; row: Opportunity } | null>(null);
  const [selectedTypeIds, setSelectedTypeIds] = useState<Set<number>>(() => new Set());
  const [lastSelectedTypeId, setLastSelectedTypeId] = useState<number | null>(null);
  const selectedTypeIdsRef = useRef(selectedTypeIds);
  const parentRef = useRef<HTMLDivElement>(null);
  selectedTypeIdsRef.current = selectedTypeIds;

  function applySelectedTypeIds(next: Set<number>) {
    selectedTypeIdsRef.current = next;
    setSelectedTypeIds(next);
  }

  const columns = useMemo<ColumnDef<Opportunity>[]>(() => [
    { accessorKey: "status", header: "Status", size: 130 },
    { accessorKey: "direction", header: "Direction", size: 130 },
    { accessorKey: "typeId", header: "Type ID", size: 90 },
    { accessorKey: "itemName", header: "Item Name", size: 260 },
    { accessorKey: "buyPrice", header: "Buy Price", size: 120, cell: ({ getValue }) => formatIsk(getValue<number | null>()) },
    { accessorKey: "sellReference", header: "Sell Ref", size: 120, cell: ({ getValue }) => formatIsk(getValue<number | null>()) },
    { accessorKey: "spread", header: "Spread", size: 95, cell: ({ getValue }) => formatPercent(getValue<number | null>()) },
    { accessorKey: "sourceAvailable", header: "Source Avail", size: 120, cell: ({ getValue }) => formatIsk(getValue<number | null>()) },
    { accessorKey: "suggestedBuyQuantity", header: "Suggested Buy", size: 125, cell: ({ getValue }) => formatQuantity(getValue<number | null>()) },
    { accessorKey: "estimatedProfit", header: "Est. Profit", size: 130, cell: ({ getValue }) => formatIsk(getValue<number | null>()) },
    { accessorKey: "cargoUsedPercent", header: "Cargo Used", size: 115, cell: ({ getValue }) => formatPercent(getValue<number | null>()) },
    { accessorKey: "myDestinationSellQuantity", header: "My Dest Qty", size: 120, cell: ({ getValue }) => formatQuantity(getValue<number | null>()) },
    { id: "myDestinationSellPrice", header: "My Dest Price", size: 140, cell: ({ row }) => formatPriceRange(row.original.myDestinationSellPriceMin, row.original.myDestinationSellPriceMax) },
    { accessorKey: "buyRegionVolume", header: "Buy 30d Vol", size: 120, cell: ({ getValue }) => formatIsk(getValue<number | null>()) },
    { accessorKey: "sellRegionVolume", header: "Sell 30d Vol", size: 120, cell: ({ getValue }) => formatIsk(getValue<number | null>()) },
    { accessorKey: "lastRefreshMinutes", header: "Last Refresh", size: 120, cell: ({ getValue }) => getValue<number | null>() === null ? "" : `${getValue<number>()} min ago` },
    { accessorKey: "notes", header: "My Notes", size: 180 }
  ], []);

  const table = useReactTable({
    data: rows,
    columns,
    state: { sorting, columnSizing },
    onSortingChange: setSorting,
    onColumnSizingChange: (updater) => {
      setColumnSizing((current) => {
        const next = typeof updater === "function" ? updater(current) : updater;
        localStorage.setItem(columnSizingStorageKey, JSON.stringify(next));
        return next;
      });
    },
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    columnResizeMode: "onEnd"
  });

  const tableRows = table.getRowModel().rows;
  const virtualizer = useVirtualizer({
    count: tableRows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 34,
    overscan: 12
  });

  const measuredRows = virtualizer.getVirtualItems();
  const virtualRows = measuredRows.length
    ? measuredRows
    : tableRows.slice(0, 50).map((_, index) => ({ index, start: index * 34, size: 34, key: index }));
  const totalSize = virtualizer.getTotalSize();
  const selectedForMenu = menu && selectedTypeIds.has(menu.row.typeId)
    ? Array.from(selectedTypeIds)
    : menu
      ? [menu.row.typeId]
      : [];

  return (
    <section className="table-shell" onClick={() => setMenu(null)}>
      <div className="table-scroll" ref={parentRef}>
        <table style={{ width: table.getTotalSize() }}>
          <thead>
            {table.getHeaderGroups().map((group) => (
              <tr key={group.id}>
                {group.headers.map((header) => (
                  <th
                    key={header.id}
                    style={{ width: header.getSize() }}
                  >
                    <button className="column-header-button" onClick={header.column.getToggleSortingHandler()} title="Click to sort">
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
                      onMouseDown={header.getResizeHandler()}
                      onTouchStart={header.getResizeHandler()}
                      title="Drag to resize column"
                    />
                  </th>
                ))}
              </tr>
            ))}
          </thead>
          <tbody style={{ height: totalSize }}>
            {virtualRows.map((virtualRow) => {
              const row = tableRows[virtualRow.index];
              const original = row.original;
              return (
                <tr
                  key={row.id}
                  className={`${selectedTypeIds.has(original.typeId) ? "is-selected " : ""}status-${original.status.toLowerCase().replaceAll(" ", "-")}`}
                  style={{ transform: `translateY(${virtualRow.start}px)` }}
                  onClick={(event) => handleRowClick(event, original, tableRows.map((tableRow) => tableRow.original), selectedTypeIdsRef.current, applySelectedTypeIds, lastSelectedTypeId, setLastSelectedTypeId)}
                  onContextMenu={(event) => {
                    event.preventDefault();
                    if (!selectedTypeIdsRef.current.has(original.typeId)) {
                      applySelectedTypeIds(new Set([original.typeId]));
                      setLastSelectedTypeId(original.typeId);
                    }
                    setMenu({ x: event.clientX, y: event.clientY, row: original });
                  }}
                >
                  {row.getVisibleCells().map((cell) => (
                    <td
                      key={cell.id}
                      className={`cell-${cell.column.id}`}
                      style={{ width: cell.column.getSize(), ...cellColor(cell.column.id, original) }}
                    >
                      {flexRender(cell.column.columnDef.cell, cell.getContext())}
                    </td>
                  ))}
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      {menu ? (
        <div className="context-menu" style={{ left: menu.x, top: menu.y }} onClick={(event) => event.stopPropagation()}>
          <button onClick={() => onRefreshRow(menu.row.typeId).finally(() => setMenu(null))}>Update data</button>
          <button onClick={() => onRefreshRows(selectedForMenu).finally(() => setMenu(null))}>Update Selected ({selectedForMenu.length})</button>
          <button onClick={() => onEditNotes(menu.row.typeId, menu.row.notes).finally(() => setMenu(null))}>Edit notes</button>
          <button onClick={() => onDisableProduct(menu.row.typeId).finally(() => setMenu(null))}>Disable product</button>
          <button onClick={() => window.open(`https://everef.net/type/${menu.row.typeId}`, "_blank")}>Open EVE item reference</button>
        </div>
      ) : null}
    </section>
  );
}

function handleRowClick(
  event: MouseEvent<HTMLTableRowElement>,
  row: Opportunity,
  visibleRows: Opportunity[],
  selectedTypeIds: Set<number>,
  setSelectedTypeIds: (next: Set<number>) => void,
  lastSelectedTypeId: number | null,
  setLastSelectedTypeId: Dispatch<SetStateAction<number | null>>
) {
  if (event.shiftKey && lastSelectedTypeId !== null) {
    const start = visibleRows.findIndex((visible) => visible.typeId === lastSelectedTypeId);
    const end = visibleRows.findIndex((visible) => visible.typeId === row.typeId);
    if (start >= 0 && end >= 0) {
      const [from, to] = start < end ? [start, end] : [end, start];
      setSelectedTypeIds(new Set(visibleRows.slice(from, to + 1).map((visible) => visible.typeId)));
      return;
    }
  }
  if (event.ctrlKey || event.metaKey) {
    const next = new Set(selectedTypeIds);
    if (next.has(row.typeId)) next.delete(row.typeId);
    else next.add(row.typeId);
    setSelectedTypeIds(next);
    setLastSelectedTypeId(row.typeId);
    return;
  }
  setSelectedTypeIds(new Set([row.typeId]));
  setLastSelectedTypeId(row.typeId);
}

function cellColor(columnId: string, row: Opportunity): CSSProperties {
  if (columnId === "status") return { backgroundColor: statusColor(row.status), fontWeight: 650 };
  if (columnId === "direction") {
    if (row.direction === "Jita -> Amarr") return { backgroundColor: "#dbeafe" };
    if (row.direction === "Amarr -> Jita") return { backgroundColor: "#ffedd5" };
  }
  if (columnId === "spread") return { backgroundColor: greenScale(row.spread ?? 0, 0.2, 1.0) };
  if (columnId === "suggestedBuyQuantity") return row.suggestedBuyQuantity ? { backgroundColor: "#ecfccb" } : {};
  if (columnId === "estimatedProfit") return { backgroundColor: greenScale(row.estimatedProfit ?? 0, 500000, 100000000) };
  if (columnId === "cargoUsedPercent") return { backgroundColor: greenScale(row.cargoUsedPercent ?? 0, 0.25, 1.0) };
  if (columnId === "myDestinationSellQuantity") {
    return row.myDestinationSellQuantity ? { backgroundColor: "#dbeafe" } : {};
  }
  if (columnId === "myDestinationSellPrice") {
    return myDestinationPriceColor(row);
  }
  if (columnId === "lastRefreshMinutes") return { backgroundColor: refreshScale(row.lastRefreshMinutes) };
  return {};
}

function myDestinationPriceColor(row: Opportunity): CSSProperties {
  if (!row.myDestinationSellQuantity || row.myDestinationSellPriceMin === null) {
    return {};
  }
  const publicPrice = row.destinationLowestSell ?? row.sellReference;
  if (publicPrice === null) return {};
  const undercut = row.myDestinationSellPriceMin > publicPrice + 0.01;
  return { backgroundColor: undercut ? "#fde2e2" : "#dcfce7", fontWeight: 650 };
}

function readSavedColumnSizing(): ColumnSizingState {
  try {
    const saved = localStorage.getItem(columnSizingStorageKey);
    return saved ? JSON.parse(saved) as ColumnSizingState : {};
  } catch {
    return {};
  }
}

function formatQuantity(value: number | null): string {
  if (value === null || Number.isNaN(value) || value <= 0) return "";
  return new Intl.NumberFormat("en-US", { maximumFractionDigits: 0 }).format(value);
}

function formatPriceRange(min: number | null, max: number | null): string {
  if (min === null || Number.isNaN(min)) return "";
  if (max !== null && !Number.isNaN(max) && max !== min) {
    return `${formatIsk(min)} - ${formatIsk(max)}`;
  }
  return formatIsk(min);
}

function statusColor(status: Opportunity["status"]): string {
  if (status === "GOOD") return "#dcfce7";
  if (status === "LOW SPREAD" || status === "LOW PROFIT" || status === "LOW TRAFFIC") return "#fef3c7";
  if (status.startsWith("NO ") || status === "ERROR") return "#fee2e2";
  return "#f3f4f6";
}

function greenScale(value: number, low: number, high: number): string {
  if (!value || value < low) return "#ffffff";
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
