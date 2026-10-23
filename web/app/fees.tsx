"use client";

import { useCallback, useEffect, useState } from "react";
import { PRICE_SCALE, formatPrice, getFeeState, type FeeState } from "@/lib/fees";
import { EXPLORER_ACCOUNT, fromStroops } from "@/lib/stellar";
import type { VaultConfig } from "@/lib/vaults";

// Meme cadence que le panneau de repartition : les frais se cristallisent au
// `checkpoint` quotidien et aux operations des autres porteurs.
const REFRESH_MS = 10_000;

function amount(value: bigint, code: string) {
  return `${Number(fromStroops(value)).toLocaleString("en-US", {
    maximumFractionDigits: 7,
  })} ${code}`;
}

// Reglage des frais d'une instance D6b et position de la tresorerie : prix
// par part face au high-water mark, ce qui dit si une commission de
// performance est due au prochain prelevement.
export function FeePanel({
  vault,
  refreshKey = 0,
}: {
  vault: VaultConfig;
  refreshKey?: number;
}) {
  const [state, setState] = useState<FeeState | null>(null);
  const [failed, setFailed] = useState(false);
  const code = vault.asset.code;

  const load = useCallback(async () => {
    try {
      setState(await getFeeState(vault));
      setFailed(false);
    } catch (e) {
      console.error("fee read failed", e);
      setFailed(true);
    }
  }, [vault]);

  useEffect(() => {
    setState(null);
  }, [vault]);

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
        <div className="allocation-head">Fees</div>
        <div className="muted-line">
          {failed ? "Fee settings unavailable, retrying." : "Reading fee settings on-chain..."}
        </div>
      </div>
    );
  }

  const { config } = state;
  if (!config) {
    return (
      <div className="allocation">
        <div className="allocation-head">Fees</div>
        <div className="muted-line">No fee configured on this instance.</div>
      </div>
    );
  }

  const treasuryValue =
    state.totalShares > 0n ? (state.treasuryShares * state.totalAssets) / state.totalShares : 0n;
  // Commission due au prochain prelevement, avec la troncature du contrat :
  // un prix a peine au-dessus du HWM (reliquat d'arrondi) ne doit rien.
  const perfDue =
    state.price > config.hwm
      ? (((state.price - config.hwm) * state.totalShares) / PRICE_SCALE) *
          BigInt(config.perfBps) /
        10_000n
      : 0n;

  return (
    <div className="allocation">
      <div className="allocation-head">
        <span>Fees</span>
        <span className="value">paid in vault shares</span>
      </div>
      <table className="pockets">
        <tbody>
          <tr>
            <td>Performance fee</td>
            <td>{config.perfBps / 100} % of the gain above the high-water mark</td>
          </tr>
          <tr>
            <td>Management fee</td>
            <td>{config.mgmtBps / 100} % a year, pro rata temporis</td>
          </tr>
          <tr>
            <td>NAV per share</td>
            <td>{state.totalShares > 0n ? formatPrice(state.price) : "-"}</td>
          </tr>
          <tr>
            <td>High-water mark</td>
            <td>{formatPrice(config.hwm)}</td>
          </tr>
          <tr>
            <td>Performance fee due</td>
            <td>{amount(perfDue, code)}</td>
          </tr>
          <tr>
            <td>Treasury</td>
            <td>
              <a
                className="mono"
                href={EXPLORER_ACCOUNT(config.treasury)}
                target="_blank"
                rel="noreferrer"
                title={config.treasury}
              >
                {config.treasury.slice(0, 4)}...{config.treasury.slice(-4)}
              </a>{" "}
              holds {amount(treasuryValue, code)}
            </td>
          </tr>
        </tbody>
      </table>
      <div className={perfDue > 0n ? "trigger due" : "trigger"}>
        {perfDue > 0n
          ? "NAV above the high-water mark: the next deposit, withdrawal or daily checkpoint charges the performance fee on the excess."
          : "No performance fee due: the NAV has not exceeded the high-water mark. The management fee accrues with time."}
      </div>
    </div>
  );
}
