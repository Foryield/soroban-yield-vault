"use client";

import { useCallback, useEffect, useState } from "react";
import {
  connectWallet,
  reconnectWallet,
  getAssetBalance,
  getVaultPosition,
  deposit,
  redeem,
  sharesForAmount,
  addTrustline,
  fundTestnetAccount,
  friendlyError,
  fromStroops,
  toStroops,
  AccountNotFundedError,
  EXPLORER_TX,
  IS_TESTNET,
  NETWORK_LABEL,
  type VaultPosition,
} from "@/lib/stellar";
import { VAULTS, DEFAULT_VAULT, vaultFromKey, type VaultConfig } from "@/lib/vaults";
import { AllocationPanel } from "./allocation";
import { FeePanel } from "./fees";
import { SwapPanel } from "./swap";
import { Footer, TopBar } from "./brand";
import { captureFromUrl } from "./capture";
import { SWAP_PAIR } from "@/lib/swap";

type Phase = "idle" | "signing" | "success" | "error";
type Mode = "deposit" | "redeem";

function formatAmount(value: string, digits = 4) {
  return Number(value).toLocaleString("en-US", {
    maximumFractionDigits: digits,
  });
}

// Montant decimal positif bien forme : seul cas ou l'apercu se calcule.
function isAmount(value: string) {
  return /^\d*\.?\d+$/.test(value.trim()) && Number(value) > 0;
}

// Prix d'une part, lu dans la position deja chargee (aucun appel).
function navPerShare(position: VaultPosition) {
  if (position.totalShares <= 0n) return null;
  return Number(toStroops(position.totalAssets)) / Number(position.totalShares);
}

function Kpi({
  label,
  value,
  unit,
  sub,
  primary,
}: {
  label: string;
  value: string;
  unit?: string;
  sub?: string;
  primary?: boolean;
}) {
  return (
    <div className={primary ? "kpi primary" : "kpi"}>
      <div className="kpi-label">{label}</div>
      {/* `key` : un chiffre mis a jour reapparait en fondu. */}
      <div className="kpi-value fresh" key={value}>
        {value}
        {unit && <span className="unit">{unit}</span>}
      </div>
      {sub && <div className="kpi-sub">{sub}</div>}
    </div>
  );
}

export default function Home() {
  const [vault, setVault] = useState<VaultConfig | undefined>(DEFAULT_VAULT);
  const [address, setAddress] = useState<string | null>(null);
  const [balance, setBalance] = useState<string>("0");
  const [trusted, setTrusted] = useState(true);
  const [mode, setMode] = useState<Mode>("deposit");
  const [amount, setAmount] = useState<string>("0.1");
  // Un rachat integral passe par la quantite exacte de parts detenues : le
  // detour par un montant d'actif tronque laisserait une poussiere de parts.
  const [redeemAll, setRedeemAll] = useState(false);
  const [phase, setPhase] = useState<Phase>("idle");
  const [txHash, setTxHash] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [needsFunding, setNeedsFunding] = useState(false);
  const [funding, setFunding] = useState(false);
  const [trusting, setTrusting] = useState(false);
  const [position, setPosition] = useState<VaultPosition | null>(null);
  // Incremente apres chaque operation signee : le panneau de repartition se
  // relit sans attendre son prochain cycle.
  const [operations, setOperations] = useState(0);
  // Signature en cours dans le panneau d'echange : la page ne lance ni depot,
  // ni rachat, ni changement d'onglet pendant ce temps.
  const [swapBusy, setSwapBusy] = useState(false);
  // Mode capture (dev uniquement, cf. capture.ts) : etat fige, aucun appel.
  const [captured, setCaptured] = useState(false);

  const asset = vault?.asset;
  const code = asset?.code ?? "";

  // Instance demandee par l'URL (`?vault=eurc`) : un lien envoye a un relecteur
  // ouvre directement la bonne, sans clic ni explication.
  useEffect(() => {
    const key = new URLSearchParams(window.location.search).get("vault");
    const wanted = vaultFromKey(key);
    if (wanted) setVault(wanted);

    const capture = captureFromUrl();
    if (capture) {
      setCaptured(true);
      setAddress(capture.address);
      setBalance(capture.balance);
      setTrusted(capture.trusted);
      setNeedsFunding(capture.needsFunding);
      setPosition(capture.position);
      setMode(capture.mode);
      setAmount(capture.amount);
      setPhase(capture.phase);
      setError(capture.error);
      setTxHash(capture.txHash);
    }
  }, []);

  // Solde de l'actif du vault ; bascule en mode "compte non finance" si
  // Horizon 404, et distingue l'absence de trustline d'un solde nul.
  const refreshBalance = useCallback(
    async (addr: string, v: VaultConfig) => {
      try {
        const { balance: bal, trusted: ok } = await getAssetBalance(addr, v.asset);
        setBalance(bal);
        setTrusted(ok);
        setNeedsFunding(false);
      } catch (e) {
        if (e instanceof AccountNotFundedError) {
          setBalance("0");
          setNeedsFunding(true);
          return;
        }
        throw e;
      }
    },
    [],
  );

  // Position dans le vault : lecture on-chain, silencieuse en cas d'echec.
  // Ni le solde ni le depot ne doivent dependre de cet affichage.
  const refreshPosition = useCallback(async (addr: string, v: VaultConfig) => {
    setPosition(await getVaultPosition(addr, v).catch(() => null));
  }, []);

  // Restaure la session wallet persistee au chargement (silencieux : ni
  // erreur ni prompt si aucune session ou wallet indisponible).
  useEffect(() => {
    if (captureFromUrl()) return;
    let cancelled = false;
    async function restore() {
      const addr = await reconnectWallet();
      if (!addr || cancelled) return;
      setAddress(addr);
    }
    restore().catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);

  // Solde et position se relisent a chaque changement d'instance : les deux
  // vaults ne partagent ni actif ni parts. Et apres chaque operation signee,
  // pour l'onglet affiche a ce moment-la.
  useEffect(() => {
    if (!address || !vault || captured) return;
    let cancelled = false;
    async function load(addr: string, v: VaultConfig) {
      try {
        await refreshBalance(addr, v);
      } catch {
        // erreur reseau : l'ecran garde sa derniere valeur connue
      }
      if (cancelled) return;
      await refreshPosition(addr, v);
    }
    load(address, vault).catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [address, vault, operations, captured, refreshBalance, refreshPosition]);

  function switchVault(next: VaultConfig) {
    if (next.key === vault?.key) return;
    setVault(next);
    setPosition(null);
    setBalance("0");
    setTrusted(true);
    setPhase("idle");
    setTxHash(null);
    setError(null);
    setRedeemAll(false);
    setMode("deposit");
    setAmount("0.1");
    const url = new URL(window.location.href);
    url.searchParams.set("vault", next.key);
    window.history.replaceState(null, "", url.toString());
  }

  // Le bandeau de succes nomme l'operation qui vient d'aboutir : le laisser en
  // place en changeant d'onglet le ferait mentir sur la transaction affichee.
  function switchMode(next: Mode) {
    if (next === mode) return;
    setMode(next);
    setRedeemAll(false);
    setPhase("idle");
    setTxHash(null);
    setError(null);
  }

  async function handleConnect() {
    try {
      setError(null);
      const addr = await connectWallet();
      setAddress(addr);
    } catch (e) {
      setError(friendlyError(e));
    }
  }

  async function handleFund() {
    if (!address || !vault || captured) return;
    setFunding(true);
    setError(null);
    try {
      await fundTestnetAccount(address);
      await refreshBalance(address, vault);
    } catch (e) {
      setError(friendlyError(e));
    } finally {
      setFunding(false);
    }
  }

  async function handleTrustline() {
    if (!address || !vault || captured) return;
    setTrusting(true);
    setError(null);
    try {
      await addTrustline(address, vault.asset);
      await refreshBalance(address, vault);
    } catch (e) {
      setError(friendlyError(e));
    } finally {
      setTrusting(false);
    }
  }

  async function handleSubmit() {
    if (!address || !vault || captured) return;
    setPhase("signing");
    setError(null);
    setTxHash(null);
    try {
      let hash: string;
      if (mode === "deposit") {
        hash = await deposit(address, vault, amount);
      } else {
        if (!position) throw new Error("Vault position unavailable");
        const shares = redeemAll
          ? position.shares
          : sharesForAmount(amount, position);
        hash = await redeem(address, vault, shares);
      }
      setTxHash(hash);
      setPhase("success");
      setRedeemAll(false);
      setOperations((n) => n + 1);
      await refreshBalance(address, vault);
      await refreshPosition(address, vault);
    } catch (e) {
      setError(friendlyError(e));
      setPhase("error");
    }
  }

  if (!vault) {
    return (
      <div className="page">
        <TopBar />
        <div className="card">
          <div className="title">Not configured</div>
          <div className="subtitle">
            No vault instance is configured for this network. Set the contract
            ids in the environment and rebuild.
          </div>
        </div>
      </div>
    );
  }

  const busy = phase === "signing";
  const numeric = Number(amount);
  const canDeposit =
    !!address && trusted && numeric > 0 && numeric <= Number(balance);
  const canRedeem =
    !!address &&
    !!position &&
    position.shares > 0n &&
    (redeemAll || (numeric > 0 && numeric <= Number(position.value)));
  const canSubmit = !busy && !swapBusy && (mode === "deposit" ? canDeposit : canRedeem);

  // Ce que la saisie permet, et ce qu'elle rendrait : affichage seulement, le
  // calcul reprend `sharesForAmount`, celui-la meme qu'utilise le rachat.
  const available = mode === "deposit" ? balance : (position?.value ?? "0");
  const wellFormed = isAmount(amount);
  const malformed = amount.trim() !== "" && !wellFormed;
  const overLimit = wellFormed && !redeemAll && numeric > Number(available);
  const nav = position ? navPerShare(position) : null;
  const previewShares =
    wellFormed && !overLimit && position && position.totalShares > 0n
      ? redeemAll
        ? position.shares
        : sharesForAmount(amount, position)
      : null;
  // XLM paie aussi les frais et la reserve du compte : pas de « Max » sur
  // l'actif natif, il viderait le compte de quoi signer.
  const canMax =
    mode === "redeem" ? !!position && position.shares > 0n : asset?.kind !== "native";
  const holdsShares = !!position && position.shares > 0n;

  const stepIndex = phase === "signing" ? 1 : phase === "success" ? 2 : -1;
  const steps = ["Amount", "Sign and settle", "Confirmed"];

  return (
    <div className="page">
      <TopBar wallet={address} />

      <div className="intro reveal">
        <div>
          <div className="eyebrow">Soroban YieldVault</div>
          <h1 className="page-title">{code} vault</h1>
          <p className="page-lead">{vault.subtitle}</p>
        </div>
        {VAULTS.length > 1 && (
          <div className="tabs" role="tablist" aria-label="Vault instance">
            {VAULTS.map((v) => (
              <button
                key={v.key}
                type="button"
                role="tab"
                aria-selected={v.key === vault.key}
                className={v.key === vault.key ? "tab active" : "tab"}
                onClick={() => switchVault(v)}
                disabled={busy || swapBusy}
              >
                {v.tab}
              </button>
            ))}
          </div>
        )}
      </div>

      <div className="home-grid">
        <div className="stack">
          <section className="card reveal" style={{ ["--i" as string]: 1 }}>
            {!address ? (
              <>
                <div className="card-title">Deposit into the vault</div>
                <p className="card-lead">
                  Connect a Stellar wallet to deposit {code} and receive vault shares.
                  Every operation is signed in your wallet and settles on {NETWORK_LABEL}.
                </p>
                <button onClick={handleConnect}>Connect wallet</button>
                <p className="connect-hint">Freighter, xBull, Albedo, Lobstr or Ledger</p>
              </>
            ) : needsFunding ? (
              <>
                <div className="card-title">Activate this account</div>
                {IS_TESTNET ? (
                  <>
                    <div className="status error">
                      This account isn&apos;t active on Stellar testnet yet. Fund it
                      with Friendbot to continue.
                    </div>
                    <button onClick={handleFund} disabled={funding}>
                      {funding ? (
                        <>
                          <span className="spinner" />
                          Funding...
                        </>
                      ) : (
                        "Fund with Friendbot"
                      )}
                    </button>
                  </>
                ) : (
                  <div className="status error">
                    This account isn&apos;t active on Stellar yet. Send XLM to it to
                    activate it, then reload.
                  </div>
                )}
              </>
            ) : !trusted ? (
              <>
                <div className="card-title">Enable {code}</div>
                <div className="status">
                  This account holds no {code} trustline. A Classic Stellar asset can
                  only be received once its trustline is open; this is a one-off
                  signature.
                </div>
                <button onClick={handleTrustline} disabled={trusting}>
                  {trusting ? (
                    <>
                      <span className="spinner" />
                      Opening trustline...
                    </>
                  ) : (
                    `Open ${code} trustline`
                  )}
                </button>
              </>
            ) : (
              <>
                <div className="tabs modes">
                  <button
                    type="button"
                    className={mode === "deposit" ? "tab active" : "tab"}
                    onClick={() => switchMode("deposit")}
                    disabled={busy}
                  >
                    Deposit
                  </button>
                  <button
                    type="button"
                    className={mode === "redeem" ? "tab active" : "tab"}
                    onClick={() => switchMode("redeem")}
                    disabled={busy || !holdsShares}
                  >
                    Redeem
                  </button>
                </div>

                <div className="field-head">
                  <label className="field" htmlFor="amount">
                    {mode === "deposit" ? "Amount to deposit" : "Amount to redeem"}
                  </label>
                  <span className="available">
                    {mode === "deposit" ? "Wallet" : "Position"}: {formatAmount(available)} {code}
                    {canMax && (
                      <button
                        type="button"
                        className="link"
                        onClick={() => {
                          if (mode === "redeem" && position) {
                            setAmount(position.value);
                            setRedeemAll(true);
                          } else {
                            setAmount(balance);
                          }
                        }}
                        disabled={busy}
                      >
                        Max
                      </button>
                    )}
                  </span>
                </div>
                <div className="input-wrap">
                  <input
                    id="amount"
                    type="text"
                    inputMode="decimal"
                    autoComplete="off"
                    value={amount}
                    aria-invalid={malformed || overLimit}
                    onChange={(e) => {
                      setAmount(e.target.value);
                      setRedeemAll(false);
                    }}
                    disabled={busy}
                  />
                  <span className="suffix">{code}</span>
                </div>
                {malformed && (
                  <div className="hint error">Enter a positive amount, such as 0.5.</div>
                )}
                {overLimit && (
                  <div className="hint error">
                    {mode === "deposit"
                      ? `More than the ${code} in your wallet.`
                      : "More than your position in the vault."}
                  </div>
                )}

                {previewShares !== null && previewShares > 0n && (
                  <div className="review">
                    {mode === "deposit" ? (
                      <div className="row">
                        <span className="label">You receive</span>
                        <span className="value">
                          &asymp; {formatAmount(fromStroops(previewShares))} shares
                        </span>
                      </div>
                    ) : (
                      <>
                        <div className="row">
                          <span className="label">Shares burned</span>
                          <span className="value">
                            {redeemAll ? "" : "≈ "}
                            {formatAmount(fromStroops(previewShares))}
                          </span>
                        </div>
                        <div className="row">
                          <span className="label">You receive</span>
                          <span className="value">
                            {redeemAll ? "" : "at most "}
                            {formatAmount(amount)} {code}
                          </span>
                        </div>
                      </>
                    )}
                    {nav !== null && (
                      <div className="row">
                        <span className="label">Price per share</span>
                        <span className="value">
                          {nav.toFixed(6)} {code}
                        </span>
                      </div>
                    )}
                  </div>
                )}

                <button onClick={handleSubmit} disabled={!canSubmit}>
                  {busy ? (
                    <>
                      <span className="spinner" />
                      Confirm in your wallet...
                    </>
                  ) : mode === "deposit" ? (
                    `Deposit${wellFormed && !overLimit ? ` ${formatAmount(amount)} ${code}` : ""}`
                  ) : redeemAll ? (
                    "Redeem everything"
                  ) : (
                    `Redeem${wellFormed && !overLimit ? ` ${formatAmount(amount)} ${code}` : ""}`
                  )}
                </button>

                {(stepIndex >= 0 || phase === "error") && (
                  <ol className="steps" aria-label="Operation progress">
                    {steps.map((label, i) => {
                      const state =
                        phase === "error"
                          ? i === 0
                            ? "done"
                            : i === 1
                              ? "failed"
                              : ""
                          : i < stepIndex || (phase === "success" && i === stepIndex)
                            ? "done"
                            : i === stepIndex
                              ? "current"
                              : "";
                      return (
                        <li key={label} className={state} aria-current={state === "current" ? "step" : undefined}>
                          {label}
                        </li>
                      );
                    })}
                  </ol>
                )}

                {trusted && Number(balance) === 0 && vault.faucet && mode === "deposit" && (
                  <div className="status">
                    No {code} on this account.{" "}
                    <a href={vault.faucet.url} target="_blank" rel="noreferrer">
                      {vault.faucet.label} &rarr;
                    </a>
                  </div>
                )}
              </>
            )}

            {phase === "success" && txHash && (
              <div className="status success" role="status">
                <strong>
                  {mode === "deposit" ? "Deposit" : "Redemption"} confirmed on {NETWORK_LABEL}
                </strong>
                Your position is up to date.{" "}
                <a href={EXPLORER_TX(txHash)} target="_blank" rel="noreferrer">
                  View on Stellar Expert &rarr;
                </a>
              </div>
            )}

            {error && (
              <div className="status error" role="alert">
                {error}
              </div>
            )}
          </section>

          {/* Reequilibrage D4 : sous les deux onglets dont il relie les actifs.
              Absent en mode capture, qui ne lit rien on-chain. */}
          {address && !needsFunding && !captured && SWAP_PAIR &&
            (vault.asset.code === SWAP_PAIR.from.code || vault.asset.code === SWAP_PAIR.to.code) && (
              <section className="card reveal" style={{ ["--i" as string]: 2 }}>
                <SwapPanel
                  address={address}
                  refreshKey={operations}
                  disabled={busy}
                  onBusyChange={setSwapBusy}
                  onSwapped={() => setOperations((n) => n + 1)}
                />
              </section>
            )}
        </div>

        <div className="stack">
          <section className="card reveal" style={{ ["--i" as string]: 2 }}>
            <div className="card-title">Your position</div>
            <p className="card-lead">Read on-chain from the vault contract.</p>
            {address && position ? (
              <div className="kpis">
                <Kpi
                  primary
                  label="Position value"
                  value={formatAmount(position.value)}
                  unit={code}
                  sub={`${formatAmount(fromStroops(position.shares))} shares`}
                />
                <Kpi
                  label="Share of the vault"
                  value={
                    position.totalShares > 0n
                      ? `${((Number(position.shares) / Number(position.totalShares)) * 100).toFixed(2)} %`
                      : "-"
                  }
                />
                <Kpi label="Vault total" value={formatAmount(position.totalAssets)} unit={code} />
                <Kpi
                  label="Price per share"
                  value={nav !== null ? nav.toFixed(6) : "-"}
                  unit={nav !== null ? code : undefined}
                />
              </div>
            ) : (
              <div className="empty">
                {!address
                  ? "Connect a wallet to see your position, your share of the vault and its price per share."
                  : needsFunding || !trusted
                    ? `Your position appears here once the account can hold ${code}.`
                    : "Reading your position on-chain..."}
              </div>
            )}
          </section>

          {vault.audited && (
            <section className="card reveal" style={{ ["--i" as string]: 3 }}>
              {vault.allocator && <AllocationPanel vault={vault} refreshKey={operations} />}
              {vault.fees && <FeePanel vault={vault} refreshKey={operations} />}
              <a className="dashboard-link" href={`/dashboard/?vault=${vault.key}`}>
                Compliance dashboard: audit trail, {vault.fees ? "fees" : "allocations"}, NAV &rarr;
              </a>
            </section>
          )}
        </div>
      </div>

      <Footer />
    </div>
  );
}
