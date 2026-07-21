import express from "express";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { loadConfig } from "../config.js";
import { dfnsClient } from "../dfns.js";
import { provisionWallet, fundWithFriendbot } from "../provision.js";
import { buildInvocationHex, depositArgs } from "../envelope.js";
import { submitViaDfns, waitForInclusion } from "../submit.js";

// Local-only demo façade for the walkthrough video. The browser talks to
// localhost and never sees DFNS credentials: they stay in this process.
export type DemoBricks = {
  // provision + fund
  onboardWallet: (email: string) => Promise<{ walletId: string; address: string }>;
  // envelope + submit + inclusion
  deposit: (
    address: string,
    walletId: string,
    stroops: bigint,
  ) => Promise<{ txHash: string; ledger: number; successful: boolean }>;
};

// Fixed server-side for the demo: 0.1 XLM, the page never chooses the amount.
const DEPOSIT_STROOPS = 1_000_000n;

const indexHtmlPath = fileURLToPath(new URL("./index.html", import.meta.url));

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function createDemoServer(bricks: DemoBricks): express.Express {
  const app = express();
  app.use(express.json());

  app.get("/", (_req, res) => {
    res.sendFile(indexHtmlPath);
  });

  app.post("/api/onboard", async (req, res) => {
    const email = req.body?.email;
    if (typeof email !== "string" || email.trim() === "") {
      res.status(400).json({ error: "email is required" });
      return;
    }
    try {
      const { walletId, address } = await bricks.onboardWallet(email.trim());
      res.json({ walletId, address });
    } catch (error) {
      // Message only, never a stack: this JSON ends up on the filmed page.
      res.status(500).json({ error: errorMessage(error) });
    }
  });

  app.post("/api/deposit", async (req, res) => {
    const { walletId, address } = req.body ?? {};
    if (typeof walletId !== "string" || walletId === "" || typeof address !== "string" || address === "") {
      res.status(400).json({ error: "walletId and address are required" });
      return;
    }
    try {
      const { txHash, ledger, successful } = await bricks.deposit(address, walletId, DEPOSIT_STROOPS);
      res.json({
        txHash,
        ledger,
        successful,
        explorerUrl: `https://stellar.expert/explorer/testnet/tx/${txHash}`,
      });
    } catch (error) {
      res.status(500).json({ error: errorMessage(error) });
    }
  });

  return app;
}

// Run directly (npm run demo): wire the real bricks, exactly like cli/onboard.ts.
// The envelope is built at deposit time so its sequence number is fresh.
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const cfg = loadConfig();
  const client = dfnsClient(cfg);
  const app = createDemoServer({
    onboardWallet: async (email) => {
      const wallet = await provisionWallet(client, email);
      await fundWithFriendbot(wallet.address);
      return wallet;
    },
    deposit: async (address, walletId, stroops) => {
      const { hex } = await buildInvocationHex({
        contractId: cfg.vaultContractId,
        method: "deposit",
        args: depositArgs(address, stroops),
        source: address,
        rpcUrl: cfg.rpcUrl,
      });
      const { txHash } = await submitViaDfns(client, walletId, hex);
      const { successful, ledger } = await waitForInclusion(cfg.horizonUrl, txHash);
      return { txHash, ledger, successful };
    },
  });

  const port = 4600;
  app.listen(port, () => {
    console.log(`For Yield demo: http://localhost:${port}`);
  });
}
