import {
  Contract,
  TransactionBuilder,
  BASE_FEE,
  Networks,
  Address,
  nativeToScVal,
  rpc,
  Horizon,
  NotFoundError,
} from "@stellar/stellar-sdk";
import {
  StellarWalletsKit,
  WalletNetwork,
  allowAllModules,
  parseError,
  FREIGHTER_ID,
} from "@creit.tech/stellar-wallets-kit";
import { LedgerModule } from "@creit.tech/stellar-wallets-kit/modules/ledger.module";

// --- Configuration reseau ---------------------------------------------------
// NEXT_PUBLIC_STELLAR_NETWORK selectionne le reseau : "testnet" (defaut) ou
// "mainnet". Passphrase, endpoints, explorer et Friendbot en decoulent.
// Les env NEXT_PUBLIC_* restent prioritaires sur les defauts publics.
// Sur mainnet, VAULT_ID et RPC_URL n'ont aucun defaut : ils DOIVENT etre
// fournis par l'environnement (fail-closed, jamais de contrat implicite).

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

const PASSPHRASE = IS_TESTNET ? Networks.TESTNET : Networks.PUBLIC;

const VAULT_ID =
  process.env.NEXT_PUBLIC_VAULT_ID ||
  (IS_TESTNET ? "CCKW7NFKDCOTOVUODLJ6K734ZEYT4TZLQGLIVFZZR6DLUHO6UOTENWQ6" : "");
const RPC_URL =
  process.env.NEXT_PUBLIC_RPC_URL ||
  (IS_TESTNET ? "https://soroban-testnet.stellar.org" : "");
const HORIZON_URL =
  process.env.NEXT_PUBLIC_HORIZON_URL ||
  (IS_TESTNET
    ? "https://horizon-testnet.stellar.org"
    : "https://horizon.stellar.org");

function requireConfig(value: string, name: string): string {
  if (!value) {
    throw new Error(`${name} must be configured for ${NETWORK}`);
  }
  return value;
}

const DECIMALS = 7;
const EXPLORER_SEGMENT = IS_TESTNET ? "testnet" : "public";

export const EXPLORER_TX = (hash: string) =>
  `https://stellar.expert/explorer/${EXPLORER_SEGMENT}/tx/${hash}`;

// Levee quand le compte du wallet n'existe pas encore on-chain (Horizon 404).
// Un compte Stellar n'existe qu'apres avoir ete finance.
export class AccountNotFundedError extends Error {
  constructor() {
    super(`Account not funded on Stellar ${NETWORK}`);
    this.name = "AccountNotFundedError";
  }
}

// Finance un compte via Friendbot. Testnet uniquement : sur mainnet le
// financement est un vrai transfert de fonds, jamais automatise ici.
export async function fundTestnetAccount(address: string): Promise<void> {
  if (!IS_TESTNET) {
    throw new Error("Friendbot is only available on testnet");
  }
  const res = await fetch(
    `https://friendbot.stellar.org/?addr=${encodeURIComponent(address)}`,
  );
  if (!res.ok) {
    throw new Error("Friendbot funding failed");
  }
}

// --- Wallet kit (multi-wallet + session) ------------------------------------
// allowAllModules() charge les wallets sans configuration prealable
// (Freighter, xBull, Albedo, Lobstr, Rabet, Hana, ...) ; Ledger exige un
// module explicite (transport WebUSB) et est ajoute a part.

const WALLET_STORAGE_KEY = "foryield:walletId";

function storedWalletId(): string | null {
  if (typeof window === "undefined") return null;
  try {
    return window.localStorage.getItem(WALLET_STORAGE_KEY);
  } catch {
    return null;
  }
}

function storeWalletId(id: string | null): void {
  if (typeof window === "undefined") return;
  try {
    if (id) {
      window.localStorage.setItem(WALLET_STORAGE_KEY, id);
    } else {
      window.localStorage.removeItem(WALLET_STORAGE_KEY);
    }
  } catch {
    // stockage indisponible (navigation privee) : session non persistee
  }
}

let kit: StellarWalletsKit | null = null;

function getKit(): StellarWalletsKit {
  if (!kit) {
    kit = new StellarWalletsKit({
      network: IS_TESTNET ? WalletNetwork.TESTNET : WalletNetwork.PUBLIC,
      selectedWalletId: storedWalletId() || FREIGHTER_ID,
      modules: [...allowAllModules(), new LedgerModule()],
    });
  }
  return kit;
}

// Traduit les erreurs kit/wallet en message actionnable pour l'UI.
// parseError vient du kit et normalise les shapes d'erreur par wallet.
export function friendlyError(e: unknown): string {
  if (e instanceof AccountNotFundedError) {
    return e.message;
  }
  let message = "";
  try {
    const parsed = parseError(e);
    message = String(parsed?.message ?? "");
  } catch {
    message = "";
  }
  if (!message) {
    message = e instanceof Error ? e.message : String(e ?? "Unknown error");
  }
  const lower = message.toLowerCase();
  if (
    lower.includes("declined") ||
    lower.includes("denied") ||
    lower.includes("reject") ||
    lower.includes("cancel")
  ) {
    return "Request declined in the wallet.";
  }
  if (lower.includes("not currently connected") || lower.includes("locked")) {
    return "Wallet locked or disconnected. Open it and reconnect.";
  }
  return message;
}

export async function connectWallet(): Promise<string> {
  const k = getKit();
  return new Promise<string>((resolve, reject) => {
    k.openModal({
      onWalletSelected: async (option) => {
        try {
          k.setWallet(option.id);
          const { address } = await k.getAddress();
          storeWalletId(option.id);
          resolve(address);
        } catch (e) {
          reject(e);
        }
      },
      onClosed: () => reject(new Error("Connection cancelled")),
    });
  });
}

// Restaure silencieusement la session wallet persistee (rechargement de page).
// Retourne null si aucune session ou si le wallet ne repond plus ; dans ce
// cas la session est purgee pour ne pas re-echouer a chaque chargement.
export async function reconnectWallet(): Promise<string | null> {
  const id = storedWalletId();
  if (!id) return null;
  try {
    const k = getKit();
    k.setWallet(id);
    const { address } = await k.getAddress();
    return address;
  } catch {
    storeWalletId(null);
    return null;
  }
}

// Oublie la session persistee et deselectionne le wallet.
export async function disconnectWallet(): Promise<void> {
  storeWalletId(null);
  if (kit) {
    try {
      await kit.disconnect();
    } catch {
      // certains wallets n'ont pas d'etat a deconnecter
    }
  }
}

export async function getNativeBalance(address: string): Promise<string> {
  const horizon = new Horizon.Server(HORIZON_URL);
  let acc: Awaited<ReturnType<typeof horizon.loadAccount>>;
  try {
    acc = await horizon.loadAccount(address);
  } catch (e) {
    if (e instanceof NotFoundError) {
      throw new AccountNotFundedError();
    }
    throw e;
  }
  const line = acc.balances.find((b) => b.asset_type === "native");
  return line ? line.balance : "0";
}

function toStroops(amount: string): bigint {
  const [intPart, fracPart = ""] = amount.trim().split(".");
  const frac = (fracPart + "0".repeat(DECIMALS)).slice(0, DECIMALS);
  return (
    BigInt(intPart || "0") * 10n ** BigInt(DECIMALS) + BigInt(frac || "0")
  );
}

// Verifie que le wallet actif est toujours celui attendu juste avant de
// signer. Le connect initial ne garantit pas une autorisation vivante au
// moment de signer (cle en cache, allowlist revoquee, compte change).
async function ensureWalletAccess(address: string): Promise<void> {
  const k = getKit();
  const { address: active } = await k.getAddress();
  if (active !== address) {
    throw new Error(
      "Active wallet account changed. Reconnect your wallet and retry.",
    );
  }
}

export async function deposit(
  address: string,
  amountAsset: string,
): Promise<string> {
  const server = new rpc.Server(requireConfig(RPC_URL, "NEXT_PUBLIC_RPC_URL"));
  const account = await server.getAccount(address);
  const contract = new Contract(requireConfig(VAULT_ID, "NEXT_PUBLIC_VAULT_ID"));

  const op = contract.call(
    "deposit",
    new Address(address).toScVal(),
    nativeToScVal(toStroops(amountAsset), { type: "i128" }),
  );

  const built = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: PASSPHRASE,
  })
    .addOperation(op)
    .setTimeout(60)
    .build();

  const prepared = await server.prepareTransaction(built);

  // Garantit une autorisation wallet vivante au moment de signer.
  await ensureWalletAccess(address);

  const k = getKit();
  const { signedTxXdr } = await k.signTransaction(prepared.toXDR(), {
    address,
    networkPassphrase: PASSPHRASE,
  });

  const signed = TransactionBuilder.fromXDR(signedTxXdr, PASSPHRASE);
  const sent = await server.sendTransaction(signed);
  if (sent.status === "ERROR") {
    throw new Error("Failed to send transaction");
  }

  let result = await server.getTransaction(sent.hash);
  let tries = 0;
  while (result.status === rpc.Api.GetTransactionStatus.NOT_FOUND && tries < 30) {
    await new Promise((r) => setTimeout(r, 1000));
    result = await server.getTransaction(sent.hash);
    tries++;
  }
  if (result.status !== rpc.Api.GetTransactionStatus.SUCCESS) {
    throw new Error("Transaction not confirmed");
  }
  return sent.hash;
}
