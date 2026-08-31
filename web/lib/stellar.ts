import {
  Asset,
  Contract,
  Operation,
  TransactionBuilder,
  BASE_FEE,
  Address,
  nativeToScVal,
  scValToNative,
  rpc,
  Horizon,
  NotFoundError,
  xdr,
} from "@stellar/stellar-sdk";
import {
  StellarWalletsKit,
  WalletNetwork,
  parseError,
  FREIGHTER_ID,
} from "@creit.tech/stellar-wallets-kit";
import { FreighterModule } from "@creit.tech/stellar-wallets-kit/modules/freighter.module";
import { xBullModule } from "@creit.tech/stellar-wallets-kit/modules/xbull.module";
import { AlbedoModule } from "@creit.tech/stellar-wallets-kit/modules/albedo.module";
import { LobstrModule } from "@creit.tech/stellar-wallets-kit/modules/lobstr.module";
import { LedgerModule } from "@creit.tech/stellar-wallets-kit/modules/ledger.module";
import {
  DECIMALS,
  HORIZON_URL,
  IS_TESTNET,
  NETWORK,
  PASSPHRASE,
  RPC_URL,
  requireConfig,
} from "./network";
import type { AssetDescriptor, VaultConfig } from "./vaults";

export { EXPLORER_TX, IS_TESTNET, NETWORK_LABEL } from "./network";
export type { StellarNetwork } from "./network";

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
// Friendbot ne distribue que du XLM : un actif classique comme EURC vient de
// son propre faucet (cf. `faucet` dans vaults.ts).
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
// Les CINQ wallets sur lesquels le livrable D2 engage la demo (cf.
// docs/evidence/d2-wallet-onboarding.md) sont charges nommement, et eux seuls.
//
// allowAllModules() en chargeait huit : Rabet, Hana, Klever et HotWallet en
// plus, jamais promis a personne, et HotWallet embarque son propre SDK. Chaque
// module retire est autant de code tiers qui ne s'execute plus dans le
// navigateur d'un utilisateur venu signer une transaction.
//
// Deux attentes a ne PAS avoir, l'une et l'autre mesurees :
//
// - `npm audit` ne bouge pas d'un pouce. @trezor/connect-web, @walletconnect/*
//   et @hot-wallet/sdk sont des dependances DURES du paquet du kit, donc
//   installees quoi qu'on importe. Elles ne sont d'ailleurs PAS dans le bundle,
//   ni avant ni apres : zero occurrence de trezor et de walletconnect dans les
//   chunks servis, dans les deux etats.
// - la taille servie ne bouge pas non plus : 2,506 Mo avant, 2,500 Mo apres.
//   Les 2,5 Mo viennent de @stellar/stellar-sdk et de la pile @ledgerhq, pas
//   des modules de portefeuille, qui pesent quelques kilo-octets chacun.
//
// Ce qui change reellement : quatre modules de moins a instancier, dont le seul
// qui tirait un SDK tiers, et une liste qui correspond enfin a l'engagement D2
// au lieu d'etre « ce que le kit expedie ce mois-ci ».
//
// Ledger exige un module explicite (transport WebUSB) et l'a toujours ete.

const WALLET_STORAGE_KEY = "foryield:walletId";

// Wallet actif en memoire : le stockage local peut etre indisponible
// (navigation privee) alors que la session, elle, est bien etablie.
let activeWalletId: string | null = null;

function storedWalletId(): string | null {
  if (typeof window === "undefined") return null;
  try {
    return window.localStorage.getItem(WALLET_STORAGE_KEY);
  } catch {
    return null;
  }
}

function storeWalletId(id: string | null): void {
  activeWalletId = id;
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

// Wallet effectivement selectionne, avec la meme valeur de repli que le kit.
function currentWalletId(): string {
  return activeWalletId || storedWalletId() || FREIGHTER_ID;
}

let kit: StellarWalletsKit | null = null;

function getKit(): StellarWalletsKit {
  if (!kit) {
    kit = new StellarWalletsKit({
      network: IS_TESTNET ? WalletNetwork.TESTNET : WalletNetwork.PUBLIC,
      selectedWalletId: storedWalletId() || FREIGHTER_ID,
      modules: [
        new FreighterModule(),
        new xBullModule(),
        new AlbedoModule(),
        new LobstrModule(),
        new LedgerModule(),
      ],
    });
  }
  return kit;
}

// Traduit les erreurs kit/wallet en message actionnable pour l'UI.
// parseError vient du kit et normalise les shapes d'erreur par wallet.
export function friendlyError(e: unknown): string {
  if (e instanceof AccountNotFundedError || e instanceof MissingTrustlineError) {
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
    activeWalletId = id;
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

// --- Soldes -----------------------------------------------------------------
// Un actif classique (EURC) n'existe sur un compte qu'a travers une trustline.
// Pas de trustline, pas de ligne de solde chez Horizon : ce n'est pas un solde
// nul, c'est un compte qui ne peut RIEN recevoir. La distinction se perdrait
// dans un "0" et le depot echouerait au moment de signer, sans explication.

export class MissingTrustlineError extends Error {
  constructor(code: string) {
    super(`No ${code} trustline on this account`);
    this.name = "MissingTrustlineError";
  }
}

export type AssetBalance = {
  /// Solde disponible, en unites d'actif ("12.5000000").
  balance: string;
  /// Faux pour un actif classique dont la trustline manque.
  trusted: boolean;
};

function horizonServer(): Horizon.Server {
  return new Horizon.Server(HORIZON_URL);
}

async function loadAccount(address: string) {
  const horizon = horizonServer();
  try {
    return await horizon.loadAccount(address);
  } catch (e) {
    if (e instanceof NotFoundError) {
      throw new AccountNotFundedError();
    }
    throw e;
  }
}

export async function getAssetBalance(
  address: string,
  asset: AssetDescriptor,
): Promise<AssetBalance> {
  const acc = await loadAccount(address);
  if (asset.kind === "native") {
    const line = acc.balances.find((b) => b.asset_type === "native");
    return { balance: line ? line.balance : "0", trusted: true };
  }
  const line = acc.balances.find(
    (b) =>
      (b.asset_type === "credit_alphanum4" ||
        b.asset_type === "credit_alphanum12") &&
      b.asset_code === asset.code &&
      b.asset_issuer === asset.issuer,
  );
  if (!line) {
    return { balance: "0", trusted: false };
  }
  return { balance: line.balance, trusted: true };
}

// Ouvre la trustline de l'actif classique : operation Stellar Classic, donc
// construite et soumise via Horizon, pas via le RPC Soroban. Le wallet la
// signe comme n'importe quelle transaction.
export async function addTrustline(
  address: string,
  asset: AssetDescriptor,
): Promise<string> {
  if (asset.kind !== "classic") {
    throw new Error("The native asset needs no trustline");
  }
  const horizon = horizonServer();
  const account = await loadAccount(address);

  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: PASSPHRASE,
  })
    .addOperation(
      Operation.changeTrust({ asset: new Asset(asset.code, asset.issuer) }),
    )
    .setTimeout(TX_TIMEOUT_S)
    .build();

  await ensureWalletAccess(address);
  const signedTxXdr = await signWithDeadline(tx.toXDR(), address);
  const signed = TransactionBuilder.fromXDR(signedTxXdr, PASSPHRASE);
  const sent = await horizon.submitTransaction(signed);
  return sent.hash;
}

function toStroops(amount: string): bigint {
  const [intPart, fracPart = ""] = amount.trim().split(".");
  const frac = (fracPart + "0".repeat(DECIMALS)).slice(0, DECIMALS);
  return (
    BigInt(intPart || "0") * 10n ** BigInt(DECIMALS) + BigInt(frac || "0")
  );
}

function fromStroops(value: bigint): string {
  const unit = 10n ** BigInt(DECIMALS);
  const negative = value < 0n;
  const abs = negative ? -value : value;
  const frac = (abs % unit).toString().padStart(DECIMALS, "0");
  return `${negative ? "-" : ""}${abs / unit}.${frac}`;
}

// Position d'un compte dans le vault. `value` est la contrepartie en actif des
// parts detenues : c'est ce qu'un retrait total rendrait aujourd'hui.
// `shares` est la quantite brute de parts, seule unite que `withdraw` accepte.
export type VaultPosition = {
  value: string;
  totalAssets: string;
  shares: bigint;
  totalShares: bigint;
};

// Lecture seule : simuler un appel n'emet aucune transaction et ne coute rien.
// Le compte source ne sert qu'a construire une enveloppe valide.
async function simulateRead(
  server: rpc.Server,
  account: Awaited<ReturnType<rpc.Server["getAccount"]>>,
  contract: Contract,
  fn: string,
  ...args: xdr.ScVal[]
): Promise<bigint> {
  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: PASSPHRASE,
  })
    .addOperation(contract.call(fn, ...args))
    .setTimeout(30)
    .build();

  const sim = await server.simulateTransaction(tx);
  if (rpc.Api.isSimulationError(sim)) {
    throw new Error(`Vault read failed (${fn}): ${sim.error}`);
  }
  if (!sim.result) {
    throw new Error(`Vault read returned nothing (${fn})`);
  }
  return BigInt(scValToNative(sim.result.retval));
}

function rpcServer(): rpc.Server {
  return new rpc.Server(requireConfig(RPC_URL, "NEXT_PUBLIC_RPC_URL"));
}

function vaultContract(vault: VaultConfig): Contract {
  return new Contract(
    requireConfig(vault.contractId, `contract id for the ${vault.tab} vault`),
  );
}

export async function getVaultPosition(
  address: string,
  vault: VaultConfig,
): Promise<VaultPosition> {
  const server = rpcServer();
  const contract = vaultContract(vault);
  const account = await server.getAccount(address);

  const [shares, totalShares, totalAssets] = await Promise.all([
    simulateRead(
      server,
      account,
      contract,
      "shares_of",
      new Address(address).toScVal(),
    ),
    simulateRead(server, account, contract, "total_shares"),
    simulateRead(server, account, contract, "total_assets"),
  ]);

  // Meme calcul tronque que le contrat au retrait : parts x actifs / total.
  const value = totalShares > 0n ? (shares * totalAssets) / totalShares : 0n;

  return {
    value: fromStroops(value),
    totalAssets: fromStroops(totalAssets),
    shares,
    totalShares,
  };
}

// Parts a bruler pour recuperer environ `amountAsset` : l'inverse du calcul du
// contrat, tronque dans le meme sens. Le rachat rendra donc au plus le montant
// demande, jamais davantage.
export function sharesForAmount(
  amountAsset: string,
  position: VaultPosition,
): bigint {
  const totalAssets = toStroops(position.totalAssets);
  if (totalAssets <= 0n) return 0n;
  return (toStroops(amountAsset) * position.totalShares) / totalAssets;
}

// Freighter peut renvoyer une cle publique en cache sans autorisation vivante
// ("<domaine> is not currently connected") : on redemande l'acces juste avant
// de signer, et requestAccess revient sans prompt si le domaine est deja
// autorise.
//
// Ce controle est volontairement limite a Freighter. Les wallets web (Albedo
// et consorts) implementent getAddress par une fenetre popup : l'appeler ici
// consomme l'activation utilisateur transitoire, le navigateur bloque alors la
// popup de signature qui suit, et le SDK du wallet reste sans reponse - depot
// impossible, aucune erreur remontee. Les wallets extension (Freighter, Hana)
// n'ouvrent pas de fenetre et ne sont pas concernes.
async function ensureWalletAccess(address: string): Promise<void> {
  if (currentWalletId() !== FREIGHTER_ID) return;
  const k = getKit();
  const { address: active } = await k.getAddress();
  if (active !== address) {
    throw new Error(
      "Active wallet account changed. Reconnect your wallet and retry.",
    );
  }
}

// Un wallet web signe dans une fenetre popup. Si le navigateur la bloque ou si
// l'utilisateur la ferme, certains SDK ne rejettent jamais leur promesse :
// sans borne, l'interface resterait en attente indefiniment sans rien
// afficher. La borne reste inferieure au timebound de la transaction, donc
// toute signature acceptee ici est encore soumissible.
const SIGN_TIMEOUT_MS = 180_000;
const TX_TIMEOUT_S = 300;

async function signWithDeadline(
  xdrToSign: string,
  address: string,
): Promise<string> {
  const k = getKit();
  let timer: ReturnType<typeof setTimeout> | undefined;
  const deadline = new Promise<never>((_, reject) => {
    timer = setTimeout(
      () =>
        reject(
          new Error(
            "No response from the wallet. If a signing window was blocked, allow pop-ups for this site, then try again.",
          ),
        ),
      SIGN_TIMEOUT_MS,
    );
  });
  try {
    const { signedTxXdr } = await Promise.race([
      k.signTransaction(xdrToSign, {
        address,
        networkPassphrase: PASSPHRASE,
      }),
      deadline,
    ]);
    return signedTxXdr;
  } finally {
    clearTimeout(timer);
  }
}

// Construit, fait signer, soumet et attend la confirmation d'un appel de
// contrat. Depot et rachat ne different que par la fonction et ses arguments.
async function invokeVault(
  address: string,
  vault: VaultConfig,
  fn: "deposit" | "withdraw",
  arg: bigint,
): Promise<string> {
  const server = rpcServer();
  const account = await server.getAccount(address);
  const contract = vaultContract(vault);

  const op = contract.call(
    fn,
    new Address(address).toScVal(),
    nativeToScVal(arg, { type: "i128" }),
  );

  const built = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: PASSPHRASE,
  })
    .addOperation(op)
    // Un wallet web demande de se connecter avant de signer : 60 s ne suffit
    // pas et la transaction serait rejetee en txTooLate.
    .setTimeout(TX_TIMEOUT_S)
    .build();

  const prepared = await server.prepareTransaction(built);

  // Garantit une autorisation Freighter vivante au moment de signer.
  await ensureWalletAccess(address);

  const signedTxXdr = await signWithDeadline(prepared.toXDR(), address);

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

export async function deposit(
  address: string,
  vault: VaultConfig,
  amountAsset: string,
): Promise<string> {
  // Le contrat appelle transfer sur le SAC de l'actif : sans trustline, la
  // simulation echoue sur une erreur d'hote illisible. Le dire avant de faire
  // signer quoi que ce soit.
  if (vault.asset.kind === "classic") {
    const { trusted } = await getAssetBalance(address, vault.asset);
    if (!trusted) throw new MissingTrustlineError(vault.asset.code);
  }
  return invokeVault(address, vault, "deposit", toStroops(amountAsset));
}

// Rachat : le contrat ne connait que les PARTS, jamais un montant d'actif.
// L'appelant convertit avec sharesForAmount, ou passe la totalite de sa
// position pour un rachat integral sans poussiere.
export async function redeem(
  address: string,
  vault: VaultConfig,
  shares: bigint,
): Promise<string> {
  if (shares <= 0n) {
    throw new Error("Nothing to redeem for this amount");
  }
  return invokeVault(address, vault, "withdraw", shares);
}
