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
  AccountNotFundedError,
  EXPLORER_TX,
  IS_TESTNET,
  NETWORK_LABEL,
  type VaultPosition,
} from "@/lib/stellar";
import { VAULTS, DEFAULT_VAULT, vaultFromKey, type VaultConfig } from "@/lib/vaults";

type Phase = "idle" | "signing" | "success" | "error";
type Mode = "deposit" | "redeem";

function shorten(addr: string) {
  return `${addr.slice(0, 4)}...${addr.slice(-4)}`;
}

function formatAmount(value: string, digits = 4) {
  return Number(value).toLocaleString("en-US", {
    maximumFractionDigits: digits,
  });
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

  const asset = vault?.asset;
  const code = asset?.code ?? "";

  // Instance demandee par l'URL (`?vault=eurc`) : un lien envoye a un relecteur
  // ouvre directement la bonne, sans clic ni explication.
  useEffect(() => {
    const key = new URLSearchParams(window.location.search).get("vault");
    const wanted = vaultFromKey(key);
    if (wanted) setVault(wanted);
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
  // vaults ne partagent ni actif ni parts.
  useEffect(() => {
    if (!address || !vault) return;
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
  }, [address, vault, refreshBalance, refreshPosition]);

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
    if (!address || !vault) return;
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
    if (!address || !vault) return;
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
    if (!address || !vault) return;
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
      await refreshBalance(address, vault);
      await refreshPosition(address, vault);
    } catch (e) {
      setError(friendlyError(e));
      setPhase("error");
    }
  }

  if (!vault) {
    return (
      <div className="shell">
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
  const canSubmit = !busy && (mode === "deposit" ? canDeposit : canRedeem);

  return (
    <div className="shell">
      <div className="brand">
        <div className="logo">
          For<span>Yield</span> &times; Stellar
        </div>
        <div className="badge">{NETWORK_LABEL}</div>
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
              disabled={busy}
            >
              {v.tab}
            </button>
          ))}
        </div>
      )}

      <div className="card">
        <div className="title">YieldVault</div>
        <div className="subtitle">{vault.subtitle}</div>

        {!address ? (
          <button onClick={handleConnect}>Connect Wallet</button>
        ) : needsFunding ? (
          <>
            <div className="row">
              <span className="label">Wallet</span>
              <span className="value mono">{shorten(address)}</span>
            </div>
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
        ) : (
          <>
            <div className="row">
              <span className="label">Wallet</span>
              <span className="value mono">{shorten(address)}</span>
            </div>
            <div className="row">
              <span className="label">{code} balance</span>
              <span className="value">
                {trusted ? `${formatAmount(balance)} ${code}` : "no trustline"}
              </span>
            </div>

            {position && (
              <>
                <div className="row">
                  <span className="label">Your vault position</span>
                  <span className="value">
                    {formatAmount(position.value)} {code}
                  </span>
                </div>
                <div className="row">
                  <span className="label">Vault total</span>
                  <span className="value">
                    {formatAmount(position.totalAssets)} {code}
                  </span>
                </div>
              </>
            )}

            {!trusted ? (
              <>
                <div className="status error">
                  This account holds no {code} trustline. A Classic Stellar
                  asset can only be received once its trustline is open; this
                  is a one-off signature.
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
                    disabled={busy}
                  >
                    Redeem
                  </button>
                </div>

                <label className="field">
                  {mode === "deposit"
                    ? "Amount to deposit"
                    : "Amount to redeem"}
                </label>
                <div className="input-wrap">
                  <input
                    type="text"
                    inputMode="decimal"
                    value={amount}
                    onChange={(e) => {
                      setAmount(e.target.value);
                      setRedeemAll(false);
                    }}
                    disabled={busy}
                  />
                  <span className="suffix">{code}</span>
                </div>

                {mode === "redeem" && position && position.shares > 0n && (
                  <button
                    type="button"
                    className="secondary"
                    onClick={() => {
                      setAmount(position.value);
                      setRedeemAll(true);
                    }}
                    disabled={busy}
                  >
                    Redeem everything ({formatAmount(position.value)} {code})
                  </button>
                )}

                <button onClick={handleSubmit} disabled={!canSubmit}>
                  {busy ? (
                    <>
                      <span className="spinner" />
                      Confirm in your wallet...
                    </>
                  ) : mode === "deposit" ? (
                    "Deposit"
                  ) : (
                    "Redeem"
                  )}
                </button>
              </>
            )}

            {trusted && Number(balance) === 0 && vault.faucet && (
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
          <div className="status success">
            {mode === "deposit" ? "Deposit" : "Redemption"} confirmed on{" "}
            {NETWORK_LABEL}. Your vault position above is up to date.
            <br />
            <a href={EXPLORER_TX(txHash)} target="_blank" rel="noreferrer">
              View on Stellar Expert &rarr;
            </a>
          </div>
        )}

        {error && <div className="status error">{error}</div>}
      </div>

      <div className="footer">
        Testnet demo - Stellar Community Fund Build - for-yield.com
        <br />
        Testnet tokens only, with no value. ForYield is not an authorised
        crypto-asset service provider; this page is not an offer of a financial
        service.
      </div>
    </div>
  );
}
