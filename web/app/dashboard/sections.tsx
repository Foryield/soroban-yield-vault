// Sections du dashboard de conformite : affichage seul, les donnees viennent
// de la page (piste d'audit fusionnee archive + RPC, etat de l'allocateur).

import { useMemo, useState } from "react";
import { protocolLabel, type StrategyConfig } from "@/lib/allocator";
import { ALGORITHMS, TRADING_CAPACITY, unitPrices } from "@/lib/compliance";
import { field, type AuditFeed, type ChainEvent } from "@/lib/events";
import { EXPLORER_ACCOUNT, EXPLORER_TX, fromStroops } from "@/lib/stellar";
import { StepChart, type Point } from "./chart";

// Libelles des adresses connues de l'instance : le vault, ses strategies, et
// les comptes qui l'ont gouverne (admin, keeper), deduits des events.
export type Labels = Map<string, string>;

export function buildLabels(
  vault: string,
  strategies: StrategyConfig[],
  events: ChainEvent[],
): Labels {
  const labels: Labels = new Map([[vault, "Vault"]]);
  for (const s of strategies) labels.set(s.address, `${protocolLabel(s.protocol)} strategy`);
  for (const e of events) {
    const admin = field<string>(e, "admin");
    if (admin) labels.set(admin, "Admin");
    if (e.name === "keeper_set") {
      const next = field<string>(e, "new");
      if (next) labels.set(next, "Keeper");
    }
  }
  return labels;
}

export function short(address: string) {
  return `${address.slice(0, 4)}...${address.slice(-4)}`;
}

export function Party({ address, labels }: { address?: string; labels: Labels }) {
  if (!address) return <span className="muted">-</span>;
  const label = labels.get(address);
  return (
    <a
      className="mono"
      href={EXPLORER_ACCOUNT(address)}
      target="_blank"
      rel="noreferrer"
      title={address}
    >
      {label ? `${label} ${short(address)}` : short(address)}
    </a>
  );
}

export function TxLink({ hash }: { hash: string }) {
  return (
    <a className="mono" href={EXPLORER_TX(hash)} target="_blank" rel="noreferrer" title={hash}>
      {hash.slice(0, 8)}
    </a>
  );
}

export function utc(iso: string) {
  return `${iso.slice(0, 10)} ${iso.slice(11, 19)}`;
}

export function units(value: unknown, code = "") {
  if (value === undefined || value === null) return "-";
  const n = Number(fromStroops(BigInt(value as bigint)));
  const text = n.toLocaleString("en-US", { maximumFractionDigits: 7 });
  return code ? `${text} ${code}` : text;
}

// --- Piste d'audit -----------------------------------------------------------

function partyOf(e: ChainEvent): string | undefined {
  return (
    field<string>(e, "owner") ??
    field<string>(e, "strategy") ??
    field<string>(e, "admin") ??
    undefined
  );
}

function decisionOf(e: ChainEvent, labels: Labels) {
  if (e.name === "deposit" || e.name === "withdraw") {
    return <span className="muted">client</span>;
  }
  const by = field<string>(e, "decided_by") ?? field<string>(e, "admin");
  const algo = field<string>(e, "algo");
  const reason = field<string>(e, "reason");
  if (!by) return <span className="muted">-</span>;
  return (
    <>
      <Party address={by} labels={labels} />
      {(algo || reason) && (
        <div className="sub">
          {[algo, reason].filter(Boolean).join(" · ")}
        </div>
      )}
    </>
  );
}

export function AuditTrail({
  events,
  labels,
  code,
}: {
  events: ChainEvent[];
  labels: Labels;
  code: string;
}) {
  const names = useMemo(() => Array.from(new Set(events.map((e) => e.name))), [events]);
  const prices = useMemo(() => unitPrices(events), [events]);
  const [shown, setShown] = useState<string | null>(null);
  const rows = events
    .filter((e) => shown === null || e.name === shown)
    .slice()
    .reverse();

  return (
    <>
      <div className="chips" role="group" aria-label="Filter by event">
        <button
          type="button"
          className={shown === null ? "chip active" : "chip"}
          onClick={() => setShown(null)}
        >
          All ({events.length})
        </button>
        {names.map((name) => (
          <button
            key={name}
            type="button"
            className={shown === name ? "chip active" : "chip"}
            onClick={() => setShown(name)}
          >
            {name} ({events.filter((e) => e.name === name).length})
          </button>
        ))}
      </div>
      <div className="table-wrap">
        <table className="grid">
          <thead>
            <tr>
              <th>Seq</th>
              <th>Time (UTC)</th>
              <th>Event</th>
              <th>Party</th>
              <th className="num">Assets</th>
              <th className="num">Shares</th>
              <th className="num">NAV / share</th>
              <th>Decided by</th>
              <th>Tx</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((e) => {
              const price = prices.get(e.id);
              return (
                <tr key={e.id}>
                  <td className="num">{e.seq?.toString() ?? "-"}</td>
                  <td className="mono">{utc(e.closedAt)}</td>
                  <td>
                    <span className={`tag ${e.name}`}>{e.name}</span>
                    {field<string>(e, "protocol") && (
                      <div className="sub">{protocolLabel(field<string>(e, "protocol")!)}</div>
                    )}
                  </td>
                  <td>
                    <Party address={partyOf(e)} labels={labels} />
                  </td>
                  <td className="num">
                    {units(field(e, "assets") ?? field(e, "assets_recovered"), code)}
                  </td>
                  <td className="num">{units(field(e, "shares"))}</td>
                  <td className="num">{price === undefined ? "-" : price.toFixed(7)}</td>
                  <td>{decisionOf(e, labels)}</td>
                  <td>
                    <TxLink hash={e.txHash} />
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </>
  );
}

// --- Allocations par lot -----------------------------------------------------

export function Allocations({
  events,
  labels,
  code,
}: {
  events: ChainEvent[];
  labels: Labels;
  code: string;
}) {
  const batches = useMemo(() => {
    const byTx = new Map<string, ChainEvent[]>();
    for (const e of events) {
      if (e.name !== "allocate" && e.name !== "deallocate") continue;
      byTx.set(e.txHash, [...(byTx.get(e.txHash) ?? []), e]);
    }
    return Array.from(byTx.values()).reverse();
  }, [events]);

  if (batches.length === 0) {
    return <div className="muted-line">No allocation yet: deposits are still idle.</div>;
  }

  return (
    <div className="table-wrap">
      <table className="grid">
        <thead>
          <tr>
            <th>Time (UTC)</th>
            <th>Moves</th>
            <th>Decided by</th>
            <th>Algorithm · reason</th>
            <th>Tx</th>
          </tr>
        </thead>
        <tbody>
          {batches.map((batch) => {
            const first = batch[0];
            return (
              <tr key={first.txHash}>
                <td className="mono">{utc(first.closedAt)}</td>
                <td>
                  {batch.map((e) => {
                    const sign = e.name === "allocate" ? "+" : "-";
                    return (
                      <div key={e.id}>
                        {protocolLabel(field<string>(e, "protocol") ?? "")} {sign}
                        {units(field(e, "assets"), code)}
                        <span className="sub inline">
                          {" "}
                          position after {units(field(e, "position_after"), code)}
                        </span>
                      </div>
                    );
                  })}
                </td>
                <td>
                  <Party address={field<string>(first, "decided_by")} labels={labels} />
                </td>
                <td>
                  {field<string>(first, "algo") ?? "client"} · {field<string>(first, "reason")}
                </td>
                <td>
                  <TxLink hash={first.txHash} />
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

// --- Valeur liquidative et releve mensuel ------------------------------------

type Statement = {
  month: string;
  opening?: number;
  closing: number;
  aum: bigint;
  deposits: bigint;
  withdrawals: bigint;
  navPublications: number;
};

function monthlyStatements(events: ChainEvent[]): Statement[] {
  const prices = unitPrices(events);
  const out: Statement[] = [];
  let previousClosing: number | undefined;
  for (const e of events) {
    const price = prices.get(e.id);
    const month = e.closedAt.slice(0, 7);
    let current = out[out.length - 1];
    if (!current || current.month !== month) {
      if (price === undefined && !current) continue;
      current = {
        month,
        opening: previousClosing,
        closing: price ?? previousClosing ?? 0,
        aum: 0n,
        deposits: 0n,
        withdrawals: 0n,
        navPublications: 0,
      };
      out.push(current);
    }
    if (price !== undefined) {
      current.closing = price;
      previousClosing = price;
    }
    const total = field<bigint>(e, "total_assets");
    if (total !== undefined) current.aum = BigInt(total);
    if (e.name === "deposit") current.deposits += BigInt(field<bigint>(e, "assets") ?? 0n);
    if (e.name === "withdraw") current.withdrawals += BigInt(field<bigint>(e, "assets") ?? 0n);
    if (e.name === "nav") current.navPublications += 1;
  }
  return out.reverse();
}

export function NavSection({ events, code }: { events: ChainEvent[]; code: string }) {
  const prices = unitPrices(events);
  const navPoints: Point[] = [];
  const aumPoints: Point[] = [];
  for (const e of events) {
    const t = Date.parse(e.closedAt);
    const price = prices.get(e.id);
    if (price !== undefined) navPoints.push({ t, v: price });
    const total = field<bigint>(e, "total_assets");
    if (total !== undefined) aumPoints.push({ t, v: Number(fromStroops(BigInt(total))) });
  }
  const statements = monthlyStatements(events);

  return (
    <>
      <div className="charts">
        <div>
          <div className="chart-title">NAV per share</div>
          <StepChart points={navPoints} format={(v) => v.toFixed(6)} label="NAV per share" />
        </div>
        <div>
          <div className="chart-title">Assets under management ({code})</div>
          <StepChart
            points={aumPoints}
            format={(v) => v.toLocaleString("en-US", { maximumFractionDigits: 4 })}
            label="Assets under management"
          />
        </div>
      </div>
      <div className="chart-title">Monthly statement</div>
      <div className="table-wrap">
        <table className="grid">
          <thead>
            <tr>
              <th>Month</th>
              <th className="num">Opening NAV / share</th>
              <th className="num">Closing NAV / share</th>
              <th className="num">AUM at close</th>
              <th className="num">Deposits</th>
              <th className="num">Withdrawals</th>
              <th className="num">Daily NAV events</th>
            </tr>
          </thead>
          <tbody>
            {statements.map((s) => (
              <tr key={s.month}>
                <td>{s.month}</td>
                <td className="num">{s.opening === undefined ? "-" : s.opening.toFixed(7)}</td>
                <td className="num">{s.closing.toFixed(7)}</td>
                <td className="num">{units(s.aum, code)}</td>
                <td className="num">{units(s.deposits, code)}</td>
                <td className="num">{units(s.withdrawals, code)}</td>
                <td className="num">{s.navPublications}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}

// --- Gouvernance ---------------------------------------------------------------

const GOVERNANCE = new Set(["keeper_set", "targets_set", "paused", "unpaused", "emergency_exit"]);

function governanceDetail(e: ChainEvent, labels: Labels, code: string) {
  switch (e.name) {
    case "keeper_set":
      return (
        <>
          <Party address={field<string>(e, "old")} labels={labels} /> &rarr;{" "}
          <Party address={field<string>(e, "new")} labels={labels} />
        </>
      );
    case "targets_set": {
      const bps = (v: unknown) =>
        ((v as number[] | undefined) ?? []).map((n) => `${Number(n) / 100} %`).join(" / ");
      return `${bps(field(e, "old"))} → ${bps(field(e, "new"))}`;
    }
    case "emergency_exit":
      return `recovered ${units(field(e, "assets_recovered"), code)}, written off ${units(
        field(e, "written_off"),
        code,
      )}${field<boolean>(e, "write_off") ? " (loss decided by admin)" : ""}`;
    default:
      return "";
  }
}

// Event `price` de la strategie Aquarius : [nouveau prix, date du taux BCE,
// prix precedent], prix a l'echelle 1e9 (USDC pour 1 EUR de test).
function priceDetail(e: ChainEvent) {
  const [price, sourceDate, previous] = (e.data as [bigint, bigint, bigint]) ?? [];
  const scale = (v: bigint) => (Number(v) / 1e9).toFixed(4);
  const date = new Date(Number(sourceDate) * 1000).toISOString().slice(0, 10);
  return `${scale(previous)} → ${scale(price)} (ECB rate of ${date})`;
}

export function Governance({
  vaultEvents,
  strategyEvents,
  labels,
  code,
}: {
  vaultEvents: ChainEvent[];
  strategyEvents: ChainEvent[];
  labels: Labels;
  code: string;
}) {
  const rows = [
    ...vaultEvents.filter((e) => GOVERNANCE.has(e.name)),
    ...strategyEvents.filter((e) => e.name === "price" || e.name === "keeper"),
  ]
    .sort((a, b) => (a.id < b.id ? 1 : -1));

  if (rows.length === 0) {
    return <div className="muted-line">No governance event yet.</div>;
  }

  return (
    <div className="table-wrap">
      <table className="grid">
        <thead>
          <tr>
            <th>Time (UTC)</th>
            <th>Contract</th>
            <th>Event</th>
            <th>Detail</th>
            <th>Signed by</th>
            <th>Tx</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((e) => (
            <tr key={e.id}>
              <td className="mono">{utc(e.closedAt)}</td>
              <td>{labels.get(e.contract) ?? short(e.contract)}</td>
              <td>
                <span className={`tag ${e.name}`}>
                  {e.name === "price" ? "reference_price" : e.name}
                </span>
              </td>
              <td>
                {e.name === "price"
                  ? priceDetail(e)
                  : e.name === "keeper"
                    ? (
                      // Event `keeper` de la strategie : la valeur est l'adresse seule.
                      <Party address={e.data as string} labels={labels} />
                    )
                    : governanceDetail(e, labels, code)}
              </td>
              <td>
                <Party
                  address={
                    field<string>(e, "admin") ??
                    field<string>(e, "decided_by") ??
                    (e.name === "price" ? (e.topics[1] as string) : undefined)
                  }
                  labels={labels}
                />
              </td>
              <td>
                <TxLink hash={e.txHash} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

// --- Registre des algorithmes et frais ---------------------------------------

export function AlgorithmRegistry() {
  return (
    <div className="table-wrap">
      <table className="grid">
        <thead>
          <tr>
            <th>Code</th>
            <th>Vault wasm hash</th>
            <th>In service since</th>
            <th>Trading capacity</th>
            <th>Description</th>
          </tr>
        </thead>
        <tbody>
          {ALGORITHMS.map((a) => (
            <tr key={a.code}>
              <td className="mono">{a.code}</td>
              <td className="mono" title={a.wasmHash}>
                {a.wasmHash.slice(0, 12)}...
              </td>
              <td>{a.inServiceSince}</td>
              <td>{TRADING_CAPACITY}</td>
              <td className="wrap">{a.description}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export function Fees({ events, code }: { events: ChainEvent[]; code: string }) {
  const fees = events.filter((e) => e.name === "fee_minted").reverse();
  if (fees.length === 0) {
    return (
      <div className="muted-line">
        No fee event on this instance. The performance and management fee module (deliverable
        6b) emits <span className="mono">fee_minted</span> once deployed.
      </div>
    );
  }
  return (
    <div className="table-wrap">
      <table className="grid">
        <thead>
          <tr>
            <th>Time (UTC)</th>
            <th>Kind</th>
            <th className="num">Fee shares</th>
            <th className="num">Fee assets</th>
            <th className="num">Rate</th>
            <th className="num">HWM before → after</th>
            <th>Tx</th>
          </tr>
        </thead>
        <tbody>
          {fees.map((e) => (
            <tr key={e.id}>
              <td className="mono">{utc(e.closedAt)}</td>
              <td>{String(e.topics[1] ?? "")}</td>
              <td className="num">{units(field(e, "fee_shares"))}</td>
              <td className="num">{units(field(e, "fee_assets"), code)}</td>
              <td className="num">{Number(field(e, "rate_bps") ?? 0) / 100} %</td>
              <td className="num">
                {units(field(e, "hwm_before"))} → {units(field(e, "hwm_after"))}
              </td>
              <td>
                <TxLink hash={e.txHash} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

// --- Sources et completude ------------------------------------------------------

export function Sources({
  feed,
  contracts,
  labels,
  gaps,
  lastSeq,
  archiveUrl,
}: {
  feed: AuditFeed;
  contracts: string[];
  labels: Labels;
  gaps: bigint[];
  lastSeq: bigint | null;
  archiveUrl: string;
}) {
  const fromArchive = feed.events.filter((e) => e.source === "archive").length;
  return (
    <ul className="sources">
      <li className={gaps.length > 0 ? "warn" : undefined}>
        <strong>Completeness:</strong>{" "}
        {lastSeq === null
          ? "record count unavailable from the vault."
          : gaps.length === 0
            ? `vault record numbers (seq) run from 1 to ${lastSeq} without a gap.`
            : `missing seq ${gaps.join(", ")} of ${lastSeq}: an event was not ingested.`}
      </li>
      <li>
        <strong>Archive:</strong> {fromArchive} events from{" "}
        <a href={archiveUrl} target="_blank" rel="noreferrer">
          the data/events branch
        </a>{" "}
        (daily ingestion; the RPC keeps events for about 7 days only).
      </li>
      {contracts.map((c) => (
        <li key={c} className="sub-item">
          {labels.get(c) ?? short(c)}:{" "}
          {feed.archivedUntil[c] === undefined
            ? "no archive yet, read from the RPC window only."
            : `archived up to ledger ${feed.archivedUntil[c]}.`}
        </li>
      ))}
      <li>
        <strong>Live:</strong> {feed.events.length - fromArchive} more events read from the
        Soroban RPC, up to ledger {feed.lastLedger}.
      </li>
      {feed.missed.map((m) => (
        <li key={`${m.contract}-${m.fromLedger}`} className="warn">
          Ledgers {m.fromLedger} to {m.toLedger} of {short(m.contract)} left the RPC window
          before ingestion.
        </li>
      ))}
    </ul>
  );
}
