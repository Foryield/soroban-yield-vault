"use client";

import { useState } from "react";
import {
  connectWallet,
  getUsdcBalance,
  deposit,
  EXPLORER_TX,
} from "@/lib/stellar";

type Phase = "idle" | "signing" | "success" | "error";

function shorten(addr: string) {
  return `${addr.slice(0, 4)}...${addr.slice(-4)}`;
}

export default function Home() {
  const [address, setAddress] = useState<string | null>(null);
  const [balance, setBalance] = useState<string>("0");
  const [amount, setAmount] = useState<string>("0.1");
  const [phase, setPhase] = useState<Phase>("idle");
  const [txHash, setTxHash] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function handleConnect() {
    try {
      setError(null);
      const addr = await connectWallet();
      setAddress(addr);
      const bal = await getUsdcBalance(addr);
      setBalance(bal);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Connexion impossible");
    }
  }

  async function handleDeposit() {
    if (!address) return;
    setPhase("signing");
    setError(null);
    setTxHash(null);
    try {
      const hash = await deposit(address, amount);
      setTxHash(hash);
      setPhase("success");
      const bal = await getUsdcBalance(address);
      setBalance(bal);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Echec du depot");
      setPhase("error");
    }
  }

  const canDeposit =
    !!address &&
    phase !== "signing" &&
    Number(amount) > 0 &&
    Number(amount) <= Number(balance);

  return (
    <div className="shell">
      <div className="brand">
        <div className="logo">
          For<span>Yield</span> &times; Stellar
        </div>
        <div className="badge">Soroban Testnet</div>
      </div>

      <div className="card">
        <div className="title">YieldVault</div>
        <div className="subtitle">
          Deposit USDC into the Soroban YieldVault. MiCA-regulated DeFi yield,
          settled on Stellar in under five seconds.
        </div>

        {!address ? (
          <button onClick={handleConnect}>Connect Wallet</button>
        ) : (
          <>
            <div className="row">
              <span className="label">Wallet</span>
              <span className="value mono">{shorten(address)}</span>
            </div>
            <div className="row">
              <span className="label">USDC balance</span>
              <span className="value">
                {Number(balance).toLocaleString("en-US", {
                  maximumFractionDigits: 4,
                })}{" "}
                USDC
              </span>
            </div>

            <label className="field">Amount to deposit</label>
            <div className="input-wrap">
              <input
                type="text"
                inputMode="decimal"
                value={amount}
                onChange={(e) => setAmount(e.target.value)}
                disabled={phase === "signing"}
              />
              <span className="suffix">USDC</span>
            </div>

            <button onClick={handleDeposit} disabled={!canDeposit}>
              {phase === "signing" ? (
                <>
                  <span className="spinner" />
                  Confirm in Freighter...
                </>
              ) : (
                "Deposit"
              )}
            </button>
          </>
        )}

        {phase === "success" && txHash && (
          <div className="status success">
            Deposit confirmed on Stellar testnet.
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
      </div>
    </div>
  );
}
