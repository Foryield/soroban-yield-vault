// --- Piste d'audit : archive et RPC ------------------------------------------
// Le dashboard lit d'abord l'archive tenue par la tache quotidienne (branche
// `data/events`, une archive JSON par contrat), puis complete au RPC a partir
// du dernier ledger archive : l'historique au-dela des 7 jours du RPC vient
// de l'archive, l'event de la minute vient du RPC.
//
// Un seul decodage, celui-ci, pour les deux sources : l'archive garde le XDR
// brut tel que la chaine l'a emis, et son decodage generique ne sert qu'aux
// systemes tiers.

import { scValToNative, xdr } from "@stellar/stellar-sdk";
import { fetchContractEvents, type RawEvent } from "./rpc-events.mjs";
import { rpcServer } from "./stellar";

// Base des archives : `<base>/<contrat>.json`. Le depot est public et
// raw.githubusercontent.com repond aux requetes du navigateur.
export const ARCHIVE_URL =
  process.env.NEXT_PUBLIC_EVENTS_ARCHIVE_URL ||
  "https://raw.githubusercontent.com/Foryield/soroban-yield-vault/refs/heads/data/events/testnet";

export type ChainEvent = {
  id: string;
  contract: string;
  ledger: number;
  closedAt: string;
  txHash: string;
  /// Nom de l'event : premier topic (`deposit`, `allocate`, `price`...).
  name: string;
  topics: unknown[];
  /// Champs nommes pour un event `#[contractevent]`, tableau sinon.
  data: unknown;
  /// Numero d'enregistrement D6a, present sur tout event du vault.
  seq?: bigint;
  source: "archive" | "rpc";
};

type Archive = {
  contract: string;
  lastLedger: number;
  missed: { fromLedger: number; toLedger: number }[];
  events: RawEvent[];
};

export type MissedRange = { contract: string; fromLedger: number; toLedger: number };

export type AuditFeed = {
  events: ChainEvent[];
  /// Dernier ledger lu, archive et RPC confondus : point de reprise.
  lastLedger: number;
  /// Dernier ledger couvert par l'archive, par contrat (absent : aucune
  /// archive lisible pour ce contrat).
  archivedUntil: Record<string, number>;
  /// Ledgers sortis de la fenetre du RPC sans avoir ete lus.
  missed: MissedRange[];
};

// Champs `#[topic]` des events du vault, dans l'ordre des topics apres le
// nom : `#[contractevent]` les place dans les topics, pas dans la data. On
// les rattache a la data sous leur nom pour lire un event d'un seul tenant.
const TOPIC_FIELDS: Record<string, string[]> = {
  deposit: ["owner"],
  withdraw: ["owner"],
  allocate: ["strategy"],
  deallocate: ["strategy"],
  emergency_exit: ["strategy"],
  paused: ["admin"],
  unpaused: ["admin"],
  fee_minted: ["kind"],
  config_set: ["key"],
};

function decodeScVal(base64: string): unknown {
  return scValToNative(xdr.ScVal.fromXDR(base64, "base64"));
}

function toChainEvent(raw: RawEvent, source: ChainEvent["source"]): ChainEvent {
  const topics = raw.topics.map(decodeScVal);
  const name = String(topics[0] ?? "");
  let data = decodeScVal(raw.value);
  const named = TOPIC_FIELDS[name];
  if (named && data && typeof data === "object" && !Array.isArray(data)) {
    data = {
      ...Object.fromEntries(named.map((key, i) => [key, topics[i + 1]])),
      ...(data as Record<string, unknown>),
    };
  }
  const seq =
    data && typeof data === "object" && !Array.isArray(data) && "seq" in data
      ? BigInt((data as { seq: bigint | number }).seq)
      : undefined;
  return {
    id: raw.id,
    contract: raw.contract,
    ledger: raw.ledger,
    closedAt: raw.closedAt,
    txHash: raw.txHash,
    name,
    topics,
    data,
    seq,
    source,
  };
}

async function loadArchive(contract: string): Promise<Archive | null> {
  try {
    const res = await fetch(`${ARCHIVE_URL}/${contract}.json`, { cache: "no-store" });
    if (!res.ok) return null;
    return (await res.json()) as Archive;
  } catch (e) {
    console.error("event archive unavailable", contract, e);
    return null;
  }
}

function byId(a: ChainEvent, b: ChainEvent) {
  return a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
}

// Fusionne sans doublon : l'id RPC est unique (ledger, transaction,
// operation, index), et la meme transaction peut venir des deux sources.
export function mergeEvents(known: ChainEvent[], fresh: ChainEvent[]): ChainEvent[] {
  const ids = new Set(known.map((e) => e.id));
  return [...known, ...fresh.filter((e) => !ids.has(e.id))].sort(byId);
}

// Chargement complet : archives des contrats, puis RPC depuis le plus ancien
// point d'arret des archives (un contrat sans archive repart du debut de la
// fenetre du RPC).
export async function loadAuditFeed(contracts: string[]): Promise<AuditFeed> {
  const archives = await Promise.all(contracts.map(loadArchive));
  const archived = archives.flatMap((a) =>
    a ? a.events.map((e) => toChainEvent(e, "archive")) : [],
  );
  const missed: MissedRange[] = archives.flatMap((a) =>
    a ? a.missed.map((m) => ({ contract: a.contract, ...m })) : [],
  );
  const archivedUntil: Record<string, number> = {};
  for (const a of archives) if (a) archivedUntil[a.contract] = a.lastLedger;
  const from = Math.min(...contracts.map((c) => archivedUntil[c] ?? 0)) + 1;

  const page = await fetchContractEvents(rpcServer(), contracts, from);
  const live = page.events.map((e) => toChainEvent(e, "rpc"));
  // Une archive en retard au-dela de la fenetre du RPC laisse un trou reel,
  // propre a son contrat : il doit se voir.
  for (const contract of contracts) {
    const until = archivedUntil[contract];
    if (until !== undefined && page.startLedger > until + 1) {
      missed.push({ contract, fromLedger: until + 1, toLedger: page.startLedger - 1 });
    }
  }

  return {
    events: mergeEvents(archived.sort(byId), live),
    lastLedger: page.latestLedger,
    archivedUntil,
    missed,
  };
}

// Suite : seulement ce qui est arrive depuis le dernier ledger lu.
export async function refreshAuditFeed(
  feed: AuditFeed,
  contracts: string[],
): Promise<AuditFeed> {
  const page = await fetchContractEvents(rpcServer(), contracts, feed.lastLedger + 1);
  if (page.events.length === 0 && page.latestLedger === feed.lastLedger) return feed;
  // Onglet reste en arriere-plan plus longtemps que la fenetre du RPC.
  const missed =
    page.startLedger > feed.lastLedger + 1
      ? [
          ...feed.missed,
          ...contracts.map((contract) => ({
            contract,
            fromLedger: feed.lastLedger + 1,
            toLedger: page.startLedger - 1,
          })),
        ]
      : feed.missed;
  return {
    ...feed,
    missed,
    events: mergeEvents(
      feed.events,
      page.events.map((e) => toChainEvent(e, "rpc")),
    ),
    lastLedger: Math.max(feed.lastLedger, page.latestLedger),
  };
}

// Controle de completude D6a : les `seq` d'un vault vont de 1 a `last_seq`
// (lu sur le contrat) sans trou. Rend les numeros absents ; un trou signale
// un event manque par l'ingestion, y compris le dernier.
export function seqGaps(events: ChainEvent[], lastSeq: bigint): bigint[] {
  const seqs = events
    .map((e) => e.seq)
    .filter((s): s is bigint => s !== undefined)
    .sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
  const gaps: bigint[] = [];
  let expected = 1n;
  for (const s of seqs) {
    while (expected < s) gaps.push(expected++);
    if (s === expected) expected++;
  }
  while (expected <= lastSeq) gaps.push(expected++);
  return gaps;
}

/// Champ nomme d'un event `#[contractevent]`, ou undefined.
export function field<T = unknown>(event: ChainEvent, name: string): T | undefined {
  const data = event.data;
  if (!data || typeof data !== "object" || Array.isArray(data)) return undefined;
  return (data as Record<string, T>)[name];
}
