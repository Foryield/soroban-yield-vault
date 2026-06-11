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
  FREIGHTER_ID,
} from "@creit.tech/stellar-wallets-kit";

// Valeurs publiques testnet (contract IDs + endpoints). Defaut en dur pour que
// la demo marche meme si les env vars ne sont pas posees sur l'hote (Render).
// L'env reste prioritaire si elle est definie.
const VAULT_ID =
  process.env.NEXT_PUBLIC_VAULT_ID ||
  "CCKW7NFKDCOTOVUODLJ6K734ZEYT4TZLQGLIVFZZR6DLUHO6UOTENWQ6";
const RPC_URL =
  process.env.NEXT_PUBLIC_RPC_URL || "https://soroban-testnet.stellar.org";
const HORIZON_URL =
  process.env.NEXT_PUBLIC_HORIZON_URL || "https://horizon-testnet.stellar.org";
const PASSPHRASE = Networks.TESTNET;
const DECIMALS = 7;

export const EXPLORER_TX = (hash: string) =>
  `https://stellar.expert/explorer/testnet/tx/${hash}`;

// Levee quand le compte du wallet n'existe pas encore sur testnet (Horizon 404).
// Un compte Stellar n'existe on-chain qu'apres avoir ete finance (Friendbot).
export class AccountNotFundedError extends Error {
  constructor() {
    super("Account not funded on Stellar testnet");
    this.name = "AccountNotFundedError";
  }
}

// Finance un compte testnet via Friendbot (XLM uniquement, pas d'USDC).
export async function fundTestnetAccount(address: string): Promise<void> {
  const res = await fetch(
    `https://friendbot.stellar.org/?addr=${encodeURIComponent(address)}`,
  );
  if (!res.ok) {
    throw new Error("Friendbot funding failed");
  }
}

let kit: StellarWalletsKit | null = null;

function getKit(): StellarWalletsKit {
  if (!kit) {
    kit = new StellarWalletsKit({
      network: WalletNetwork.TESTNET,
      selectedWalletId: FREIGHTER_ID,
      modules: allowAllModules(),
    });
  }
  return kit;
}

export async function connectWallet(): Promise<string> {
  const k = getKit();
  return new Promise<string>((resolve, reject) => {
    k.openModal({
      onWalletSelected: async (option) => {
        try {
          k.setWallet(option.id);
          const { address } = await k.getAddress();
          resolve(address);
        } catch (e) {
          reject(e);
        }
      },
      onClosed: () => reject(new Error("Connection cancelled")),
    });
  });
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

// Re-demande l'acces a Freighter juste avant de signer. Le connect initial
// (getAddress -> requestAccess) ne garantit pas que le domaine est encore dans
// l'allowlist au moment de signer : getAddress peut renvoyer une cle publique
// en cache sans autorisation vivante, ce qui declenche le warning Freighter
// "<domaine> is not currently connected". Si le domaine est deja autorise,
// requestAccess revient sans prompt ; sinon il propose de reconnecter.
async function ensureWalletAccess(address: string): Promise<void> {
  const k = getKit();
  const { address: active } = await k.getAddress();
  if (active !== address) {
    throw new Error(
      "Active Freighter account changed. Reconnect your wallet and retry.",
    );
  }
}

export async function deposit(
  address: string,
  amountUsdc: string,
): Promise<string> {
  const server = new rpc.Server(RPC_URL);
  const account = await server.getAccount(address);
  const contract = new Contract(VAULT_ID);

  const op = contract.call(
    "deposit",
    new Address(address).toScVal(),
    nativeToScVal(toStroops(amountUsdc), { type: "i128" }),
  );

  const built = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: PASSPHRASE,
  })
    .addOperation(op)
    .setTimeout(60)
    .build();

  const prepared = await server.prepareTransaction(built);

  // Garantit une autorisation Freighter vivante au moment de signer.
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
