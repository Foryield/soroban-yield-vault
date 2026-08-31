import { Networks } from "@stellar/stellar-sdk";

// --- Configuration reseau ---------------------------------------------------
// NEXT_PUBLIC_STELLAR_NETWORK selectionne le reseau : "testnet" (defaut) ou
// "mainnet". Passphrase, endpoints, explorer et Friendbot en decoulent.
// Les env NEXT_PUBLIC_* restent prioritaires sur les defauts publics.
// Sur mainnet, RPC_URL et les identifiants de contrat (cf. vaults.ts) n'ont
// aucun defaut : ils DOIVENT etre fournis par l'environnement (fail-closed,
// jamais de contrat implicite).
//
// Ce module est separe de stellar.ts parce que vaults.ts a besoin du reseau
// pour choisir ses defauts, et que stellar.ts a besoin de vaults.ts : sans
// cette coupure, l'import serait circulaire.

export type StellarNetwork = "testnet" | "mainnet";

function resolveNetwork(raw: string | undefined): StellarNetwork {
  const value = (raw || "testnet").toLowerCase();
  if (value === "mainnet" || value === "public") return "mainnet";
  if (value === "testnet") return "testnet";
  throw new Error(`Unsupported NEXT_PUBLIC_STELLAR_NETWORK: ${raw}`);
}

export const NETWORK: StellarNetwork = resolveNetwork(
  process.env.NEXT_PUBLIC_STELLAR_NETWORK,
);
export const IS_TESTNET = NETWORK === "testnet";
export const NETWORK_LABEL = IS_TESTNET ? "Soroban Testnet" : "Soroban Mainnet";

export const PASSPHRASE = IS_TESTNET ? Networks.TESTNET : Networks.PUBLIC;

export const RPC_URL =
  process.env.NEXT_PUBLIC_RPC_URL ||
  (IS_TESTNET ? "https://soroban-testnet.stellar.org" : "");
export const HORIZON_URL =
  process.env.NEXT_PUBLIC_HORIZON_URL ||
  (IS_TESTNET
    ? "https://horizon-testnet.stellar.org"
    : "https://horizon.stellar.org");

// Tous les actifs Stellar, natif comme classiques, comptent 7 decimales : le
// wrapper SAC expose EURC dans la meme unite brute que XLM.
export const DECIMALS = 7;

const EXPLORER_SEGMENT = IS_TESTNET ? "testnet" : "public";

export const EXPLORER_TX = (hash: string) =>
  `https://stellar.expert/explorer/${EXPLORER_SEGMENT}/tx/${hash}`;

export function requireConfig(value: string, name: string): string {
  if (!value) {
    throw new Error(`${name} must be configured for ${NETWORK}`);
  }
  return value;
}
