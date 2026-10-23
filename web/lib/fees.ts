// --- Frais D6b ----------------------------------------------------------------
// Lecture du reglage des frais d'une instance (`fees`) et de ce qui en decoule
// pour l'affichage : prix par part face au high-water mark, parts et valeur
// detenues par la tresorerie. Plan : docs/plans/2026-10-23-d6b-frais.md.

import { Address } from "@stellar/stellar-sdk";
import { readContract } from "./stellar";
import type { VaultConfig } from "./vaults";

/// Echelle du prix par part du contrat (`PRICE_SCALE`) : 9 decimales.
export const PRICE_SCALE = 1_000_000_000n;

export type FeeConfig = {
  treasury: string;
  perfBps: number;
  mgmtBps: number;
  /// High-water mark, a l'echelle PRICE_SCALE.
  hwm: bigint;
};

export type FeeState = {
  config: FeeConfig | null;
  totalAssets: bigint;
  totalShares: bigint;
  /// Prix par part courant, a l'echelle PRICE_SCALE (0 sans part).
  price: bigint;
  treasuryShares: bigint;
};

type RawFeeConfig = {
  treasury: string;
  perf_bps: number;
  mgmt_bps: number;
  hwm: bigint;
};

/// Prix par part a l'echelle PRICE_SCALE, en texte a 7 decimales.
export function formatPrice(value: unknown): string {
  if (value === undefined || value === null) return "-";
  return (Number(BigInt(value as bigint)) / Number(PRICE_SCALE)).toFixed(7);
}

export async function getFeeState(vault: VaultConfig): Promise<FeeState> {
  const id = vault.contractId;
  const [raw, totalAssets, totalShares] = await Promise.all([
    readContract(id, "fees") as Promise<RawFeeConfig | undefined>,
    readContract(id, "total_assets") as Promise<bigint>,
    readContract(id, "total_shares") as Promise<bigint>,
  ]);
  const config = raw
    ? {
        treasury: raw.treasury,
        perfBps: Number(raw.perf_bps),
        mgmtBps: Number(raw.mgmt_bps),
        hwm: BigInt(raw.hwm),
      }
    : null;
  const assets = BigInt(totalAssets);
  const shares = BigInt(totalShares);
  const treasuryShares = config
    ? BigInt((await readContract(id, "shares_of", new Address(config.treasury).toScVal())) as bigint)
    : 0n;
  return {
    config,
    totalAssets: assets,
    totalShares: shares,
    price: shares > 0n ? (assets * PRICE_SCALE) / shares : 0n,
    treasuryShares,
  };
}
