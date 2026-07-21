import {
  Account,
  Address,
  BASE_FEE,
  Contract,
  Networks,
  TransactionBuilder,
  nativeToScVal,
  rpc,
  xdr,
} from "@stellar/stellar-sdk";

export type InvocationRequest = {
  contractId: string;
  method: string;
  args: xdr.ScVal[];
  source: string; // G… address; must also be the auth invoker (source-account auth)
  rpcUrl: string;
};

export type BuiltEnvelope = { xdrBase64: string; hex: string };

export function depositArgs(from: string, stroops: bigint): xdr.ScVal[] {
  return [new Address(from).toScVal(), nativeToScVal(stroops, { type: "i128" })];
}

export function withdrawArgs(from: string, shares: bigint): xdr.ScVal[] {
  return [new Address(from).toScVal(), nativeToScVal(shares, { type: "i128" })];
}

export async function buildInvocationHex(
  request: InvocationRequest,
  server: rpc.Server = new rpc.Server(request.rpcUrl),
): Promise<BuiltEnvelope> {
  const account: Account = await server.getAccount(request.source);
  const tx = new TransactionBuilder(account, { fee: BASE_FEE, networkPassphrase: Networks.TESTNET })
    .addOperation(new Contract(request.contractId).call(request.method, ...request.args))
    .setTimeout(300)
    .build();

  const simulation = await server.simulateTransaction(tx);
  if (!rpc.Api.isSimulationSuccess(simulation)) {
    throw new Error(`simulation failed: ${"error" in simulation ? simulation.error : "unknown"}`);
  }

  const assembled = rpc.assembleTransaction(tx, simulation).build();
  const raw = assembled.toEnvelope().toXDR();
  return { xdrBase64: assembled.toXDR(), hex: `0x${Buffer.from(raw).toString("hex")}` };
}
