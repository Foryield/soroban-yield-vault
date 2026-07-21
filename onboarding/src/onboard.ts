import type { BuiltEnvelope } from "./envelope.js";
import type { ProvisionedWallet } from "./provision.js";
import type { InclusionResult, SubmitResult } from "./submit.js";

export type OnboardDeps = {
  provision: (name: string) => Promise<ProvisionedWallet>;
  fund: (address: string) => Promise<void>;
  buildEnvelope: (source: string, stroops: bigint) => Promise<BuiltEnvelope>;
  submit: (walletId: string, hex: string) => Promise<SubmitResult>;
  waitForInclusion: (txHash: string) => Promise<InclusionResult>;
};

export type OnboardSummary = {
  email: string;
  walletId: string;
  address: string;
  txHash: string;
  ledger: number;
  successful: boolean;
};

// Pure orchestration: wallet -> funded account -> deposit envelope -> broadcast
// -> on-chain inclusion. The envelope is built after funding, right before
// submission, so its sequence number reflects the freshly created account.
// An unsuccessful inclusion still resolves: the caller decides how to react.
export async function onboard(email: string, stroops: bigint, deps: OnboardDeps): Promise<OnboardSummary> {
  const { walletId, address } = await deps.provision(email);
  await deps.fund(address);
  const { hex } = await deps.buildEnvelope(address, stroops);
  const { txHash } = await deps.submit(walletId, hex);
  const { successful, ledger } = await deps.waitForInclusion(txHash);
  return { email, walletId, address, txHash, ledger, successful };
}
