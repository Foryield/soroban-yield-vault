// --- Registre des instances de vault ----------------------------------------
// Une seule page sert plusieurs instances du meme contrat, une par actif. Le
// choix se fait dans l'interface et se transporte dans l'URL (`?vault=eurc`),
// pour qu'un lien pointe directement sur l'instance a montrer.
//
// Ce qui est fige ici : la DESCRIPTION des actifs. L'emetteur EURC de Circle
// sur testnet est une constante du reseau, pas une valeur de deploiement.
// Ce qui vient de l'environnement : les IDENTIFIANTS DE CONTRAT, parce qu'ils
// changent a chaque redeploiement (cf. scripts/redeploy_vault.sh).
//
// Sur mainnet, aucun identifiant n'a de defaut : une instance non configuree
// disparait du selecteur plutot que de pointer sur un contrat implicite. Meme
// regle fail-closed que le reseau et le RPC dans stellar.ts.

import { IS_TESTNET } from "./network";

export type VaultKey = "xlm" | "eurc";

// `native` = XLM, porte par le compte lui-meme. `classic` = actif Stellar
// classique detenu via une trustline, et manipule en Soroban par son
// StellarAssetContract (le wrapper SAC du livrable 3).
export type AssetDescriptor =
  | { kind: "native"; code: string }
  | { kind: "classic"; code: string; issuer: string };

export type FaucetHint = {
  label: string;
  url: string;
};

export type VaultConfig = {
  key: VaultKey;
  /// Onglet du selecteur.
  tab: string;
  contractId: string;
  asset: AssetDescriptor;
  subtitle: string;
  /// Ou se procurer l'actif de test quand Friendbot ne le distribue pas.
  faucet?: FaucetHint;
};

const TESTNET_DEFAULTS: Record<VaultKey, string> = {
  xlm: "CCP3EJYJ55RLZYCHABIWCTCWRHQN2BYZVXLCHZLPCCKIKA4VNK6TMCHN",
  eurc: "CDZR2IY4V3GXUONLTVXJNCMTIR2LLFC55ZRPPEHCTI4RM7LVF25UKG5K",
};

function contractId(key: VaultKey, fromEnv: string | undefined): string {
  return fromEnv || (IS_TESTNET ? TESTNET_DEFAULTS[key] : "");
}

// Emetteur officiel de l'EURC de Circle sur testnet. Sur mainnet, Circle emet
// depuis une autre cle : l'instance EURC mainnet devra la porter, et tant
// qu'elle n'est pas ecrite ici elle reste hors du selecteur.
const EURC_TESTNET_ISSUER =
  "GB3Q6QDZYTHWT7E5PVS3W7FUT5GVAFC5KSZFFLPU25GO7VTC3NM2ZTVO";

const ALL: VaultConfig[] = [
  {
    key: "xlm",
    tab: "XLM",
    contractId: contractId("xlm", process.env.NEXT_PUBLIC_VAULT_ID),
    asset: { kind: "native", code: "XLM" },
    subtitle:
      "Deposit XLM into the Soroban YieldVault. DeFi yield built for EU regulatory requirements, settled on Stellar in under five seconds.",
  },
  {
    key: "eurc",
    tab: "EURC",
    contractId: contractId("eurc", process.env.NEXT_PUBLIC_EURC_VAULT_ID),
    asset: IS_TESTNET
      ? { kind: "classic", code: "EURC", issuer: EURC_TESTNET_ISSUER }
      : { kind: "classic", code: "EURC", issuer: "" },
    subtitle:
      "Deposit EURC, a Classic Stellar asset held through its StellarAssetContract wrapper. The vault takes custody with no lending strategy attached, since no EURC pool exists on testnet.",
    faucet: {
      label: "Get testnet EURC from Circle",
      url: "https://faucet.circle.com/",
    },
  },
];

// Une instance sans contrat configure, ou un actif classique sans emetteur, ne
// serait qu'un onglet qui echoue au clic.
export const VAULTS: VaultConfig[] = ALL.filter(
  (v) =>
    v.contractId !== "" && (v.asset.kind === "native" || v.asset.issuer !== ""),
);

export const DEFAULT_VAULT: VaultConfig | undefined = VAULTS[0];

// Cle de vault portee par l'URL (`?vault=eurc`), pour qu'un lien envoye a un
// relecteur ouvre directement la bonne instance. Une cle inconnue retombe sur
// le defaut plutot que sur une page vide.
export function vaultFromKey(key: string | null): VaultConfig | undefined {
  if (!key) return DEFAULT_VAULT;
  return VAULTS.find((v) => v.key === key.toLowerCase()) || DEFAULT_VAULT;
}
