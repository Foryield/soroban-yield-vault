// Mode capture, pour les captures d'ecran et la relecture du design en local :
// `?capture=deposit` (etc.) pose un etat de page fige, sans wallet et sans
// aucune lecture ni transaction on-chain. Actif seulement sous `next dev` :
// au build, NODE_ENV vaut "production" et `captureFromUrl` rend toujours null,
// le reste du module disparait du bundle exporte.

import type { VaultPosition } from "@/lib/stellar";

export type CaptureState = {
  address: string;
  balance: string;
  trusted: boolean;
  needsFunding: boolean;
  position: VaultPosition | null;
  mode: "deposit" | "redeem";
  amount: string;
  phase: "idle" | "signing" | "success" | "error";
  error: string | null;
  txHash: string | null;
};

// Adresse et hash de forme valide, qui ne designent rien.
const ADDRESS = "GDEMOXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXCAPT";
const HASH = "0".repeat(64);

const POSITION: VaultPosition = {
  value: "1.2497500",
  totalAssets: "1.9996000",
  shares: 12_500_000n,
  totalShares: 20_000_000n,
};

const BASE: CaptureState = {
  address: ADDRESS,
  balance: "8.5000000",
  trusted: true,
  needsFunding: false,
  position: POSITION,
  mode: "deposit",
  amount: "0.5",
  phase: "idle",
  error: null,
  txHash: null,
};

const SCENARIOS: Record<string, Partial<CaptureState>> = {
  deposit: {},
  "deposit-signing": { phase: "signing" },
  "deposit-success": { phase: "success", txHash: HASH },
  "deposit-over": { amount: "12" },
  redeem: { mode: "redeem", amount: "0.4" },
  "redeem-success": { mode: "redeem", amount: "0.4", phase: "success", txHash: HASH },
  error: { phase: "error", error: "Request declined in the wallet." },
  "first-deposit": { position: { ...POSITION, value: "0.0000000", shares: 0n } },
  trustline: { trusted: false, balance: "0", position: null },
  fund: { needsFunding: true, balance: "0", position: null },
};

export function captureFromUrl(): CaptureState | null {
  if (process.env.NODE_ENV !== "development" || typeof window === "undefined") return null;
  const key = new URLSearchParams(window.location.search).get("capture");
  const scenario = key ? SCENARIOS[key] : undefined;
  return scenario ? { ...BASE, ...scenario } : null;
}
