import {
  Contract,
  TransactionBuilder,
  BASE_FEE,
  Networks,
  Address,
  nativeToScVal,
  rpc,
  Horizon,
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
  "CDPZCITOBYAO4SHLGMLDSK7Y7NFR4GWXCTSRKI6ZHMPHTCFVWCPADIHJ";
const USDC_ISSUER =
  process.env.NEXT_PUBLIC_USDC_ISSUER ||
  "GCHARQP3MBZJAUQJ5WHS3AF25G3Z5NP2AET34JOTWYNW75SY6QS5T5HY";
const RPC_URL =
  process.env.NEXT_PUBLIC_RPC_URL || "https://soroban-testnet.stellar.org";
const HORIZON_URL =
  process.env.NEXT_PUBLIC_HORIZON_URL || "https://horizon-testnet.stellar.org";
const PASSPHRASE = Networks.TESTNET;
const DECIMALS = 7;

export const EXPLORER_TX = (hash: string) =>
  `https://stellar.expert/explorer/testnet/tx/${hash}`;

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

export async function getUsdcBalance(address: string): Promise<string> {
  const horizon = new Horizon.Server(HORIZON_URL);
  const acc = await horizon.loadAccount(address);
  const line = acc.balances.find(
    (b) =>
      "asset_code" in b &&
      b.asset_code === "USDC" &&
      "asset_issuer" in b &&
      b.asset_issuer === USDC_ISSUER,
  );
  return line ? line.balance : "0";
}

function toStroops(amount: string): bigint {
  const [intPart, fracPart = ""] = amount.trim().split(".");
  const frac = (fracPart + "0".repeat(DECIMALS)).slice(0, DECIMALS);
  return (
    BigInt(intPart || "0") * 10n ** BigInt(DECIMALS) + BigInt(frac || "0")
  );
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
