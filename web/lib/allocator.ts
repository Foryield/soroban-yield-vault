// --- Etat de l'allocateur D5 -------------------------------------------------
// Repartition d'une instance a strategies, lue sur la chaine sans wallet :
// une poche par strategie plus la poche oisive (le solde du vault lui-meme),
// chacune face a sa cible. Sert la page produit et le dashboard.
//
// La regle de declenchement est celle du contrat (contracts/vault/src/lib.rs,
// `rebalance`), recopiee a l'identique pour que l'indicateur affiche ce que
// le keeper obtiendrait : chaque cible vaut total x cible / 10 000, tronque ;
// l'oisif vise le reste ; un reequilibrage passe des qu'une poche s'ecarte de
// sa cible de STRICTEMENT plus que total x drift_bps / 10 000.

import { readContract } from "./stellar";
import type { VaultConfig } from "./vaults";

const BPS = 10_000n;

export type StrategyConfig = {
  address: string;
  protocol: string;
  targetBps: number;
  capBps: number;
  active: boolean;
};

export type Pocket = {
  /// Adresse de la strategie, absente pour la poche oisive.
  address?: string;
  label: string;
  value: bigint;
  target: bigint;
  targetBps: number;
  active: boolean;
};

export type AllocatorState = {
  totalAssets: bigint;
  totalShares: bigint;
  pockets: Pocket[];
  driftBps: number;
  /// Ecart maximal d'une poche a sa cible, en unites d'actif.
  maxGap: bigint;
  threshold: bigint;
  /// Vrai si `rebalance` passerait maintenant (hors pause).
  triggered: boolean;
  paused: boolean;
  keeper: string;
};

// Libelle affiche d'un protocole : le contrat ne connait que son symbole.
const PROTOCOL_LABELS: Record<string, string> = {
  blend: "Blend",
  aquarius: "Aquarius",
};

export function protocolLabel(protocol: string): string {
  return PROTOCOL_LABELS[protocol] ?? protocol;
}

type RawStrategy = {
  address: string;
  protocol: string;
  target_bps: number;
  cap_bps: number;
  active: boolean;
};

export async function getStrategies(contractId: string): Promise<StrategyConfig[]> {
  const raw = (await readContract(contractId, "strategies")) as RawStrategy[];
  return raw.map((s) => ({
    address: s.address,
    protocol: s.protocol,
    targetBps: Number(s.target_bps),
    capBps: Number(s.cap_bps),
    active: s.active,
  }));
}

export async function getAllocatorState(vault: VaultConfig): Promise<AllocatorState> {
  const id = vault.contractId;
  const [strategies, allocation, totalAssets, totalShares, driftBps, paused, keeper] =
    await Promise.all([
      getStrategies(id),
      readContract(id, "allocation") as Promise<[string, bigint][]>,
      readContract(id, "total_assets") as Promise<bigint>,
      readContract(id, "total_shares") as Promise<bigint>,
      readContract(id, "drift_bps") as Promise<number>,
      readContract(id, "is_paused") as Promise<boolean>,
      readContract(id, "keeper") as Promise<string>,
    ]);

  const total = BigInt(totalAssets);
  const values = new Map(allocation.map(([address, value]) => [address, BigInt(value)]));

  const pockets: Pocket[] = strategies.map((s) => ({
    address: s.address,
    label: protocolLabel(s.protocol),
    value: values.get(s.address) ?? 0n,
    target: s.active ? (total * BigInt(s.targetBps)) / BPS : 0n,
    targetBps: s.active ? s.targetBps : 0,
    active: s.active,
  }));
  const invested = pockets.reduce((sum, p) => sum + p.value, 0n);
  const targeted = pockets.reduce((sum, p) => sum + p.target, 0n);
  const idleTargetBps =
    10_000 - pockets.reduce((sum, p) => sum + p.targetBps, 0);
  pockets.push({
    label: "Idle",
    value: total - invested,
    target: total - targeted,
    targetBps: idleTargetBps,
    active: true,
  });

  const threshold = (total * BigInt(driftBps)) / BPS;
  const maxGap = pockets.reduce((max, p) => {
    const gap = p.value > p.target ? p.value - p.target : p.target - p.value;
    return gap > max ? gap : max;
  }, 0n);

  return {
    totalAssets: total,
    totalShares: BigInt(totalShares),
    pockets,
    driftBps: Number(driftBps),
    maxGap,
    threshold,
    triggered: total > 0n && maxGap > threshold,
    paused,
    keeper,
  };
}

/// Part d'une poche dans le total, en pourcentage a une decimale.
export function share(value: bigint, total: bigint): number {
  if (total <= 0n) return 0;
  return Number((value * 1000n) / total) / 10;
}
