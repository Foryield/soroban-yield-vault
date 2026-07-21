import { loadConfig } from "../config.js";
import { dfnsClient } from "../dfns.js";
import { loadDotenv } from "../env.js";
import { buildInvocationHex, depositArgs } from "../envelope.js";
import { onboard } from "../onboard.js";
import { provisionWallet, fundWithFriendbot, type ProvisionedWallet } from "../provision.js";
import { submitViaDfns, waitForInclusion } from "../submit.js";

loadDotenv();

const [email, stroopsRaw] = process.argv.slice(2);
if (!email || !stroopsRaw) {
  console.error("Usage: npm run onboard -- <email> <amount-stroops>");
  process.exit(1);
}

let provisioned: ProvisionedWallet | undefined;
try {
  const stroops = BigInt(stroopsRaw);
  if (stroops <= 0n) throw new Error(`amount must be a positive number of stroops, got ${stroopsRaw}`);
  const cfg = loadConfig();
  const client = dfnsClient(cfg);
  const summary = await onboard(email, stroops, {
    provision: async (name) => (provisioned = await provisionWallet(client, name, cfg.network)),
    fund: fundWithFriendbot,
    buildEnvelope: (source, amount) =>
      buildInvocationHex({
        contractId: cfg.vaultContractId,
        method: "deposit",
        args: depositArgs(source, amount),
        source,
        rpcUrl: cfg.rpcUrl,
      }),
    submit: (walletId, hex) => submitViaDfns(client, walletId, hex),
    waitForInclusion: (txHash) => waitForInclusion(cfg.horizonUrl, txHash),
  });
  console.log(JSON.stringify(summary));
  if (!summary.successful) {
    console.error(`transaction ${summary.txHash} included in ledger ${summary.ledger} but failed on-chain`);
    process.exit(2);
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  if (provisioned) {
    console.error(
      `Wallet provisioned before failure: walletId=${provisioned.walletId} address=${provisioned.address}`,
    );
  }
  process.exit(1);
}
