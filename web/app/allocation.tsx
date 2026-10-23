"use client";

import { useCallback, useEffect, useState } from "react";
import { getAllocatorState, share, type AllocatorState } from "@/lib/allocator";
import { fromStroops } from "@/lib/stellar";
import type { VaultConfig } from "@/lib/vaults";

// Relecture periodique : le reequilibrage est declenche par le keeper, hors de
// cette page. Sans relecture, l'ecran ne montrerait jamais son effet.
const REFRESH_MS = 10_000;

function amount(value: bigint, code: string) {
  return `${Number(fromStroops(value)).toLocaleString("en-US", {
    maximumFractionDigits: 4,
  })} ${code}`;
}

function points(value: number) {
  return `${value > 0 ? "+" : ""}${value.toFixed(1)} pts`;
}

// Repartition d'une instance a strategies, face a ses cibles, et etat du
// declencheur de reequilibrage. `refreshKey` force une relecture immediate
// (apres un depot ou un retrait signe sur la page).
export function AllocationPanel({
  vault,
  refreshKey = 0,
}: {
  vault: VaultConfig;
  refreshKey?: number;
}) {
  const [state, setState] = useState<AllocatorState | null>(null);
  const [failed, setFailed] = useState(false);
  const code = vault.asset.code;

  const load = useCallback(async () => {
    try {
      setState(await getAllocatorState(vault));
      setFailed(false);
    } catch (e) {
      console.error("allocation read failed", e);
      setFailed(true);
    }
  }, [vault]);

  // Changement d'instance : l'ancienne repartition ne doit pas rester
  // affichee sous le nouvel onglet.
  useEffect(() => {
    setState(null);
  }, [vault]);

  // Une relecture garde l'affichage en place jusqu'a la nouvelle valeur.
  useEffect(() => {
    load();
    const timer = setInterval(() => {
      if (document.visibilityState === "visible") load();
    }, REFRESH_MS);
    return () => clearInterval(timer);
  }, [load, refreshKey]);

  if (!state) {
    return (
      <div className="allocation">
        <div className="allocation-head">Allocation</div>
        <div className="muted-line">
          {failed ? "Allocation unavailable, retrying." : "Reading allocation on-chain..."}
        </div>
      </div>
    );
  }

  const total = state.totalAssets;
  const gapPts = share(state.maxGap, total);
  const thresholdPts = state.driftBps / 100;

  return (
    <div className="allocation">
      <div className="allocation-head">
        <span>Allocation</span>
        <span className="value">{amount(total, code)}</span>
      </div>

      <div className="allocation-bar" aria-hidden="true">
        {state.pockets.map((p) => (
          <span
            key={p.label}
            className={`segment ${p.label.toLowerCase()}`}
            style={{ width: `${share(p.value, total)}%` }}
          />
        ))}
      </div>

      <table className="pockets">
        <thead>
          <tr>
            <th>Pocket</th>
            <th>Amount</th>
            <th>Share</th>
            <th>Target</th>
            <th>Gap</th>
          </tr>
        </thead>
        <tbody>
          {state.pockets.map((p) => {
            const pct = share(p.value, total);
            const target = p.targetBps / 100;
            return (
              <tr key={p.label}>
                <td>
                  <span className={`dot ${p.label.toLowerCase()}`} />
                  {p.label}
                  {!p.active && " (exited)"}
                </td>
                <td>{amount(p.value, code)}</td>
                <td>{pct.toFixed(1)} %</td>
                <td>{target} %</td>
                <td>{points(pct - target)}</td>
              </tr>
            );
          })}
        </tbody>
      </table>

      <div className={state.triggered ? "trigger due" : "trigger"}>
        {state.paused
          ? "Vault paused: deposits, withdrawals and rebalancing are frozen."
          : state.triggered
            ? `Rebalance due: a pocket is ${gapPts.toFixed(1)} pts off target, beyond the ${thresholdPts} pts trigger.`
            : `Within band: largest gap ${gapPts.toFixed(1)} pts, under the ${thresholdPts} pts trigger. No rebalance.`}
      </div>
    </div>
  );
}
