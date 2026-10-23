"use client";

// Dashboard de conformite (livrable D6c, plan docs/plans/2026-10-23-d6c-
// dashboard.md, section 6) : piste d'audit des events D6a d'une instance a
// strategies, lue dans l'archive puis au RPC, et ce qui s'en deduit
// (allocations par lot, valeur liquidative, releve mensuel, gouvernance).

import { useCallback, useEffect, useMemo, useState } from "react";
import { getStrategies, type StrategyConfig } from "@/lib/allocator";
import { registerRow, toCsv } from "@/lib/compliance";
import {
  ARCHIVE_URL,
  loadAuditFeed,
  refreshAuditFeed,
  seqGaps,
  type AuditFeed,
  type ChainEvent,
} from "@/lib/events";
import { EXPLORER_CONTRACT, NETWORK, NETWORK_LABEL } from "@/lib/stellar";
import { VAULTS, vaultFromKey, type VaultConfig } from "@/lib/vaults";
import { AllocationPanel } from "../allocation";
import {
  AlgorithmRegistry,
  Allocations,
  AuditTrail,
  Fees,
  Governance,
  NavSection,
  Sources,
  buildLabels,
} from "./sections";

// Pendant une seance filmee, un reequilibrage doit apparaitre sans recharger.
const REFRESH_MS = 15_000;

const ALLOCATOR_VAULTS = VAULTS.filter((v) => v.allocator);

function download(name: string, type: string, content: string) {
  const url = URL.createObjectURL(new Blob([content], { type }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  URL.revokeObjectURL(url);
}

// JSON exact : les i128 et u64 restent des chaines, sans perte au-dela de 2^53.
function exactJson(value: unknown) {
  return JSON.stringify(value, (_, v) => (typeof v === "bigint" ? v.toString() : v), 2);
}

export default function Dashboard() {
  const [vault, setVault] = useState<VaultConfig | undefined>(ALLOCATOR_VAULTS[0]);
  const [strategies, setStrategies] = useState<StrategyConfig[] | null>(null);
  const [feed, setFeed] = useState<AuditFeed | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const wanted = vaultFromKey(new URLSearchParams(window.location.search).get("vault"));
    if (wanted?.allocator) setVault(wanted);
  }, []);

  const contracts = useMemo(
    () => (vault && strategies ? [vault.contractId, ...strategies.map((s) => s.address)] : []),
    [vault, strategies],
  );

  useEffect(() => {
    if (!vault) return;
    let cancelled = false;
    setStrategies(null);
    setFeed(null);
    getStrategies(vault.contractId)
      .then((s) => !cancelled && setStrategies(s))
      .catch((e) => {
        console.error("strategies read failed", e);
        if (!cancelled) setError("Could not read the vault configuration from the RPC.");
      });
    return () => {
      cancelled = true;
    };
  }, [vault]);

  useEffect(() => {
    if (contracts.length === 0) return;
    let cancelled = false;
    loadAuditFeed(contracts)
      .then((f) => {
        if (!cancelled) {
          setFeed(f);
          setError(null);
        }
      })
      .catch((e) => {
        console.error("audit feed load failed", e);
        if (!cancelled) setError("Could not load the audit trail from the RPC.");
      });
    return () => {
      cancelled = true;
    };
  }, [contracts]);

  const refresh = useCallback(async () => {
    if (!feed || document.visibilityState !== "visible") return;
    try {
      const next = await refreshAuditFeed(feed, contracts);
      if (next !== feed) setFeed(next);
    } catch (e) {
      console.error("audit feed refresh failed", e);
    }
  }, [feed, contracts]);

  useEffect(() => {
    const timer = setInterval(refresh, REFRESH_MS);
    return () => clearInterval(timer);
  }, [refresh]);

  if (!vault) {
    return (
      <div className="shell">
        <div className="card">
          <div className="title">Not configured</div>
          <div className="subtitle">No multi-strategy vault is configured for this network.</div>
        </div>
      </div>
    );
  }

  const code = vault.asset.code;
  const vaultEvents: ChainEvent[] = feed
    ? feed.events.filter((e) => e.contract === vault.contractId)
    : [];
  const strategyEvents: ChainEvent[] = feed
    ? feed.events.filter((e) => e.contract !== vault.contractId)
    : [];
  const labels = buildLabels(vault.contractId, strategies ?? [], feed?.events ?? []);
  const gaps = seqGaps(vaultEvents);

  function exportCsv() {
    const rows = vaultEvents.map((e) => registerRow(e, vault!.contractId, NETWORK));
    download(`audit-trail-${vault!.key}.csv`, "text/csv", toCsv(rows));
  }

  function exportJson() {
    download(
      `audit-trail-${vault!.key}.json`,
      "application/json",
      exactJson({
        vault: vault!.contractId,
        network: NETWORK,
        exportedAt: new Date().toISOString(),
        lastLedger: feed?.lastLedger,
        register: vaultEvents.map((e) => registerRow(e, vault!.contractId, NETWORK)),
        events: feed?.events,
      }),
    );
  }

  return (
    <div className="shell wide">
      <div className="brand">
        <div className="logo">
          For<span>Yield</span> &times; Stellar
          <span className="logo-sub">Compliance dashboard</span>
        </div>
        <div className="badge">{NETWORK_LABEL}</div>
      </div>

      {ALLOCATOR_VAULTS.length > 1 && (
        <div className="tabs" role="tablist" aria-label="Vault instance">
          {ALLOCATOR_VAULTS.map((v) => (
            <button
              key={v.key}
              type="button"
              role="tab"
              aria-selected={v.key === vault.key}
              className={v.key === vault.key ? "tab active" : "tab"}
              onClick={() => setVault(v)}
            >
              {v.tab}
            </button>
          ))}
        </div>
      )}

      <div className="dash-grid">
        <section className="card">
          <div className="section-title">Vault</div>
          <div className="row">
            <span className="label">Contract</span>
            <a
              className="value mono"
              href={EXPLORER_CONTRACT(vault.contractId)}
              target="_blank"
              rel="noreferrer"
            >
              {vault.contractId.slice(0, 6)}...{vault.contractId.slice(-6)}
            </a>
          </div>
          <div className="row">
            <span className="label">Asset</span>
            <span className="value">{code}</span>
          </div>
          {(strategies ?? []).map((s) => (
            <div className="row" key={s.address}>
              <span className="label">{labels.get(s.address)}</span>
              <a
                className="value mono"
                href={EXPLORER_CONTRACT(s.address)}
                target="_blank"
                rel="noreferrer"
              >
                target {s.targetBps / 100} %, cap {s.capBps / 100} %
              </a>
            </div>
          ))}
          <div className="row">
            <span className="label">Recorded events</span>
            <span className="value">{feed ? vaultEvents.length : "..."}</span>
          </div>
          <a className="dashboard-link" href={`/?vault=${vault.key}`}>
            &larr; Deposit or withdraw
          </a>
        </section>

        <section className="card">
          <AllocationPanel vault={vault} />
        </section>
      </div>

      {error && <div className="status error">{error}</div>}

      <section className="card">
        <div className="section-head">
          <div className="section-title">Audit trail</div>
          <div className="actions">
            <button type="button" className="secondary small" onClick={exportCsv} disabled={!feed}>
              Export CSV
            </button>
            <button type="button" className="secondary small" onClick={exportJson} disabled={!feed}>
              Export JSON
            </button>
          </div>
        </div>
        <p className="note">
          Every vault event of the AMF audit-trail schema, in record order. Personal data stays
          off-chain: events carry addresses, and the client join happens in the back office.
        </p>
        {feed ? (
          <AuditTrail events={vaultEvents} labels={labels} code={code} />
        ) : (
          <div className="muted-line">Loading the audit trail...</div>
        )}
      </section>

      <section className="card">
        <div className="section-title">Allocations</div>
        <p className="note">
          One row per batch: every move of a rebalance shares one transaction hash. Withdrawals
          the idle pocket cannot cover pull from the strategies, decided by the client.
        </p>
        {feed && <Allocations events={vaultEvents} labels={labels} code={code} />}
      </section>

      <section className="card">
        <div className="section-title">Net asset value</div>
        <p className="note">
          Rebuilt from the totals every event carries, plus one nav event a day published by the
          keeper.
        </p>
        {feed && <NavSection events={vaultEvents} code={code} />}
      </section>

      <section className="card">
        <div className="section-title">Governance</div>
        <p className="note">
          Keeper and target changes keep the previous value; reference price publications of the
          Aquarius strategy are bounded to 1 % per hour.
        </p>
        {feed && (
          <Governance
            vaultEvents={vaultEvents}
            strategyEvents={strategyEvents}
            labels={labels}
            code={code}
          />
        )}
      </section>

      <div className="dash-grid">
        <section className="card">
          <div className="section-title">Decision algorithms</div>
          <AlgorithmRegistry />
        </section>
        <section className="card">
          <div className="section-title">Fees</div>
          {feed && <Fees events={vaultEvents} code={code} />}
        </section>
      </div>

      <section className="card">
        <div className="section-title">Sources</div>
        {feed ? (
          <Sources feed={feed} gaps={gaps} archiveUrl={ARCHIVE_URL} />
        ) : (
          <div className="muted-line">Reading the archive and the RPC...</div>
        )}
      </section>

      <div className="footer">
        Testnet demo - Stellar Community Fund Build - for-yield.com
        <br />
        Testnet tokens only, with no value. ForYield is not an authorised crypto-asset service
        provider; this page is not an offer of a financial service.
      </div>
    </div>
  );
}
