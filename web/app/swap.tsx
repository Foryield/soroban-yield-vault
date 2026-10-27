"use client";

import { useCallback, useEffect, useState } from "react";
import {
  EXPLORER_TX,
  addTrustline,
  friendlyError,
  fromStroops,
  getAssetBalance,
  toStroops,
  type AssetBalance,
} from "@/lib/stellar";
import {
  SLIPPAGE_BPS,
  SWAP_PAIR,
  executeSwap,
  quoteSwap,
  type SwapQuote,
  type SwapReceipt,
} from "@/lib/swap";

type Phase = "idle" | "quoting" | "signing" | "success";

function amount(value: bigint, code: string) {
  return `${Number(fromStroops(value)).toLocaleString("en-US", {
    maximumFractionDigits: 7,
  })} ${code}`;
}

// Reequilibrage USDC -> EURC du livrable 4, signe par le wallet connecte :
// cotation, minimum garanti, frais du pool, puis l'echange et ce qu'il a servi.
// `refreshKey` relit les soldes apres une operation signee ailleurs sur la page.
export function SwapPanel({
  address,
  refreshKey = 0,
  onSwapped,
}: {
  address: string;
  refreshKey?: number;
  onSwapped: () => void;
}) {
  const [balances, setBalances] = useState<[AssetBalance, AssetBalance] | null>(null);
  const [input, setInput] = useState("0.2");
  const [quote, setQuote] = useState<SwapQuote | null>(null);
  const [receipt, setReceipt] = useState<SwapReceipt | null>(null);
  const [phase, setPhase] = useState<Phase>("idle");
  const [trusting, setTrusting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadBalances = useCallback(async () => {
    if (!SWAP_PAIR) return;
    try {
      setBalances(
        await Promise.all([
          getAssetBalance(address, SWAP_PAIR.from),
          getAssetBalance(address, SWAP_PAIR.to),
        ]),
      );
    } catch (e) {
      console.error("swap balances read failed", e);
    }
  }, [address]);

  useEffect(() => {
    loadBalances();
  }, [loadBalances, refreshKey]);

  if (!SWAP_PAIR) return null;
  const { from, to } = SWAP_PAIR;
  const busy = phase === "quoting" || phase === "signing";
  const amountIn = /^\d*\.?\d+$/.test(input.trim()) ? toStroops(input) : 0n;
  const fromBalance = balances ? toStroops(balances[0].balance) : 0n;
  const toTrusted = balances?.[1].trusted ?? true;

  async function handleQuote() {
    setPhase("quoting");
    setError(null);
    setReceipt(null);
    try {
      setQuote(await quoteSwap(address, amountIn));
      setPhase("idle");
    } catch (e) {
      setError(friendlyError(e));
      setPhase("idle");
    }
  }

  async function handleSwap() {
    if (!quote) return;
    setPhase("signing");
    setError(null);
    try {
      setReceipt(await executeSwap(address, quote));
      setQuote(null);
      setPhase("success");
      await loadBalances();
      onSwapped();
    } catch (e) {
      setError(friendlyError(e));
      setPhase("idle");
    }
  }

  async function handleTrustline() {
    setTrusting(true);
    setError(null);
    try {
      await addTrustline(address, to);
      await loadBalances();
    } catch (e) {
      setError(friendlyError(e));
    } finally {
      setTrusting(false);
    }
  }

  return (
    <div className="allocation">
      <div className="allocation-head">
        <span>
          Rebalance {from.code} &rarr; {to.code}
        </span>
        <span className="value">SwapRouter, Aquarius pool</span>
      </div>
      <p className="note">
        The router swaps on the Aquarius {from.code} / {to.code} pool and serves at least the
        signed minimum, or the whole transaction reverts.
      </p>

      <table className="pockets">
        <tbody>
          <tr>
            <td>{from.code} balance</td>
            <td>{balances ? amount(fromBalance, from.code) : "..."}</td>
          </tr>
          <tr>
            <td>{to.code} balance</td>
            <td>
              {!balances
                ? "..."
                : balances[1].trusted
                  ? amount(toStroops(balances[1].balance), to.code)
                  : "no trustline"}
            </td>
          </tr>
        </tbody>
      </table>

      {!toTrusted ? (
        <>
          <div className="status error">
            The swap pays out {to.code}: open its trustline first, a one-off signature.
          </div>
          <button onClick={handleTrustline} disabled={trusting}>
            {trusting ? (
              <>
                <span className="spinner" />
                Opening trustline...
              </>
            ) : (
              `Open ${to.code} trustline`
            )}
          </button>
        </>
      ) : (
        <>
          <label className="field">Amount to swap</label>
          <div className="input-wrap">
            <input
              type="text"
              inputMode="decimal"
              value={input}
              onChange={(e) => {
                // La cotation vaut pour un montant : le changer l'efface.
                setInput(e.target.value);
                setQuote(null);
              }}
              disabled={busy}
            />
            <span className="suffix">{from.code}</span>
          </div>

          {quote && (
            <table className="pockets">
              <tbody>
                <tr>
                  <td>Quoted output</td>
                  <td>{amount(quote.amountOut, to.code)}</td>
                </tr>
                <tr>
                  <td>Guaranteed minimum (quote &minus; {Number(SLIPPAGE_BPS) / 100} %)</td>
                  <td>{amount(quote.minOut, to.code)}</td>
                </tr>
                <tr>
                  <td>Pool fee</td>
                  <td>{amount(quote.fee, from.code)}</td>
                </tr>
              </tbody>
            </table>
          )}

          {quote ? (
            <button onClick={handleSwap} disabled={busy}>
              {phase === "signing" ? (
                <>
                  <span className="spinner" />
                  Confirm in your wallet...
                </>
              ) : (
                `Swap ${amount(quote.amountIn, from.code)}`
              )}
            </button>
          ) : (
            <button
              onClick={handleQuote}
              disabled={busy || amountIn <= 0n || amountIn > fromBalance}
            >
              {phase === "quoting" ? (
                <>
                  <span className="spinner" />
                  Quoting...
                </>
              ) : (
                "Get a quote"
              )}
            </button>
          )}
        </>
      )}

      {phase === "success" && receipt && (
        <div className="status success">
          Swap confirmed: {amount(receipt.amountOut, to.code)} received, pool fee{" "}
          {amount(receipt.fee, from.code)}.
          <br />
          <a href={EXPLORER_TX(receipt.hash)} target="_blank" rel="noreferrer">
            View on Stellar Expert &rarr;
          </a>
        </div>
      )}

      {error && <div className="status error">{error}</div>}
    </div>
  );
}
