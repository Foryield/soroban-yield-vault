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
  QUOTE_TTL_MS,
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

// Saisie decimale, point ou virgule (clavier en locale francaise) ; null si
// elle ne forme pas un montant positif.
function parseAmount(input: string): bigint | null {
  const normalized = input.trim().replace(",", ".");
  if (!/^\d*\.?\d+$/.test(normalized)) return null;
  const value = toStroops(normalized);
  return value > 0n ? value : null;
}

// Reequilibrage USDC -> EURC du livrable 4, signe par le wallet connecte :
// cotation, minimum garanti, frais du pool, puis l'echange et ce qu'il a servi.
// `refreshKey` signale une operation signee ailleurs sur la page : soldes
// relus, cotation effacee. `disabled` gele le panneau pendant une signature
// de la page ; `onBusyChange` gele la page pendant celles du panneau, un seul
// compte ne pouvant signer deux transactions a la fois.
export function SwapPanel({
  address,
  refreshKey = 0,
  disabled = false,
  onBusyChange,
  onSwapped,
}: {
  address: string;
  refreshKey?: number;
  disabled?: boolean;
  onBusyChange: (busy: boolean) => void;
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
      setError("Could not read the wallet balances. Reload the page to retry.");
    }
  }, [address]);

  // Une operation signee ailleurs change les soldes : la cotation, faite sur
  // l'ancien etat, ne se signe plus.
  useEffect(() => {
    setQuote(null);
    loadBalances();
  }, [loadBalances, refreshKey]);

  const busy = phase === "quoting" || phase === "signing";
  useEffect(() => {
    onBusyChange(phase === "signing" || trusting);
  }, [phase, trusting, onBusyChange]);

  if (!SWAP_PAIR) return null;
  const { from, to } = SWAP_PAIR;
  const amountIn = parseAmount(input);
  const fromBalance = balances ? toStroops(balances[0].balance) : null;
  const toTrusted = balances?.[1].trusted ?? true;
  const insufficient = amountIn !== null && fromBalance !== null && amountIn > fromBalance;
  const locked = busy || disabled;

  async function handleQuote() {
    if (amountIn === null) return;
    setPhase("quoting");
    setError(null);
    setReceipt(null);
    try {
      setQuote(await quoteSwap(address, amountIn));
    } catch (e) {
      setError(friendlyError(e));
    } finally {
      setPhase("idle");
    }
  }

  async function handleSwap() {
    if (!quote) return;
    if (Date.now() - quote.quotedAt > QUOTE_TTL_MS) {
      setQuote(null);
      setError(`Quote expired after ${QUOTE_TTL_MS / 1000} s: get a new one.`);
      return;
    }
    setPhase("signing");
    setError(null);
    try {
      setReceipt(await executeSwap(address, quote));
      setQuote(null);
      setPhase("success");
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
            <td>{fromBalance === null ? "..." : amount(fromBalance, from.code)}</td>
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
          <button onClick={handleTrustline} disabled={trusting || disabled}>
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
              disabled={locked}
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
                  <td>Pool fee, included in the price</td>
                  <td>{amount(quote.fee, from.code)}</td>
                </tr>
              </tbody>
            </table>
          )}

          {quote ? (
            <button onClick={handleSwap} disabled={locked}>
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
            <button onClick={handleQuote} disabled={locked || amountIn === null || insufficient}>
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

          {input.trim() !== "" && amountIn === null && (
            <div className="muted-line">Enter a positive amount, such as 0.2.</div>
          )}
          {insufficient && (
            <div className="muted-line">
              Not enough {from.code} in the wallet: redeem from the {from.code} vault first.
            </div>
          )}
        </>
      )}

      {phase === "success" && receipt && (
        <div className="status success">
          {receipt.amountOut !== undefined && receipt.fee !== undefined
            ? `Swap confirmed: ${amount(receipt.amountOut, to.code)} received, pool fee ${amount(receipt.fee, from.code)}.`
            : "Swap confirmed."}
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
