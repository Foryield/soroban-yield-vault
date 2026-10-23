// --- Registre de conformite --------------------------------------------------
// Ce que le dashboard ajoute aux events bruts : le registre des algorithmes
// de decision et la mise en forme des events en lignes de registre, selon la
// correspondance du schema D6a (docs/plans/2026-10-20-d6a-schema-events-
// conformite.md, section 5). Les champs qui relevent de foryield-v2
// (identite du client, conversion en euros) restent hors de cette page.

import { protocolLabel } from "./allocator";
import { field, type ChainEvent } from "./events";
import { fromStroops } from "./stellar";

/// Algorithme de decision (champ 41 du reglement delegue 2025/1140). Le code
/// ne change que si la logique de decision change, jamais pour un parametre
/// (reponse 2 de Julien, D6a section 8).
export type Algorithm = {
  code: string;
  wasmHash: string;
  inServiceSince: string;
  description: string;
};

export const ALGORITHMS: Algorithm[] = [
  {
    code: "fy_alloc_v1",
    wasmHash: "9364a31e8d0b0467690104f4c5bb66d62ca27d6fecdc6dfdf05d9059aa2a0b2e",
    inServiceSince: "2026-10-21",
    description:
      "Drift rebalancing: when any pocket (idle included) is more than drift_bps of total assets away from its fixed target, the keeper moves every pocket back to target in one batch, withdrawals first. Loss per batch bounded at 1 % of the volume moved.",
  },
];

/// Capacite de negociation (champ 28) : le PSCA agit pour le compte du
/// client (reponse 1 de Julien).
export const TRADING_CAPACITY = "AOTC";

export type RegisterRow = {
  order_reference: string;
  execution_at: string;
  transaction_type: string;
  seq: string;
  owner: string;
  strategy: string;
  execution_platform: string;
  quantity_assets: string;
  quantity_shares: string;
  unit_price: string;
  total_assets_after: string;
  total_shares_after: string;
  decision_by: string;
  algorithm: string;
  reason: string;
  trading_capacity: string;
  blockchain_tx_hash: string;
  blockchain_network: string;
};

function amount(value: unknown): string {
  return value === undefined || value === null ? "" : fromStroops(BigInt(value as bigint));
}

/// Valeur liquidative par part apres l'operation, quand l'event porte les
/// deux totaux.
export function unitPrice(event: ChainEvent): number | undefined {
  const assets = field<bigint>(event, "total_assets");
  const shares = field<bigint>(event, "total_shares");
  if (assets === undefined || shares === undefined || BigInt(shares) === 0n) return undefined;
  return Number(BigInt(assets)) / Number(BigInt(shares));
}

/// Valeur liquidative par part apres chaque event du vault qui porte
/// `total_assets`, cles = id d'event. Les parts ne changent qu'au depot et au
/// retrait : un `allocate` ou un `deallocate` de reequilibrage, qui ne porte
/// que l'actif, se valorise avec le dernier `total_shares` connu. C'est la que
/// se lit le cout d'entree d'une strategie.
///
/// Exception : un `deallocate` de raison `redeem` sert un retrait. Il est
/// emis avant le `withdraw` et porte deja l'actif d'APRES le retrait, alors
/// que les parts brulees n'apparaissent qu'au `withdraw` : le diviser par les
/// parts d'avant afficherait un creux factice. Il ne se valorise pas.
export function unitPrices(events: ChainEvent[]): Map<string, number> {
  const prices = new Map<string, number>();
  let shares: bigint | undefined;
  for (const e of events) {
    const totalShares = field<bigint>(e, "total_shares");
    if (totalShares !== undefined) shares = BigInt(totalShares);
    if (e.name === "deallocate" && field<string>(e, "reason") === "redeem") continue;
    const assets = field<bigint>(e, "total_assets");
    if (assets !== undefined && shares !== undefined && shares > 0n) {
      prices.set(e.id, Number(BigInt(assets)) / Number(shares));
    }
  }
  return prices;
}

// Une ligne par event du vault. Un depot ou un retrait est decide par le
// client : champ 41 vide (reponse 3 de Julien). Une allocation porte le
// decideur et le code d'algorithme ; un rapatriement `redeem` est decide par
// le client, sans algorithme.
export function registerRow(event: ChainEvent, vault: string, network: string): RegisterRow {
  const decidedBy = field<string>(event, "decided_by");
  const protocol = field<string>(event, "protocol");
  const price = unitPrice(event);
  const clientDecision = event.name === "deposit" || event.name === "withdraw";
  return {
    order_reference: `${event.txHash}:${event.seq ?? ""}`,
    execution_at: event.closedAt,
    transaction_type: event.name,
    seq: event.seq?.toString() ?? "",
    owner: field<string>(event, "owner") ?? "",
    strategy: field<string>(event, "strategy") ?? "",
    execution_platform: protocol ? `${vault} via ${protocolLabel(protocol)}` : vault,
    quantity_assets: amount(field(event, "assets") ?? field(event, "assets_recovered")),
    quantity_shares: amount(field(event, "shares")),
    unit_price: price === undefined ? "" : price.toFixed(7),
    total_assets_after: amount(field(event, "total_assets")),
    total_shares_after: amount(field(event, "total_shares")),
    decision_by: clientDecision ? "" : decidedBy ?? field<string>(event, "admin") ?? "",
    algorithm: field<string>(event, "algo") ?? "",
    reason: field<string>(event, "reason") ?? "",
    trading_capacity: clientDecision ? "" : TRADING_CAPACITY,
    blockchain_tx_hash: event.txHash,
    blockchain_network: `stellar-${network}`,
  };
}

function csvCell(value: string): string {
  return /[",\n]/.test(value) ? `"${value.replace(/"/g, '""')}"` : value;
}

export function toCsv(rows: RegisterRow[]): string {
  if (rows.length === 0) return "";
  const header = Object.keys(rows[0]) as (keyof RegisterRow)[];
  return [
    header.join(","),
    ...rows.map((row) => header.map((key) => csvCell(row[key])).join(",")),
  ].join("\n");
}
