import { DfnsApiClient } from "@dfns/sdk";
import { AsymmetricKeySigner } from "@dfns/sdk-keysigner";
import type { Config } from "./config.js";

export function dfnsClient(cfg: Config): DfnsApiClient {
  return new DfnsApiClient({
    baseUrl: cfg.dfnsApiUrl,
    authToken: cfg.dfnsAuthToken,
    signer: new AsymmetricKeySigner({
      credId: cfg.dfnsCredId,
      privateKey: cfg.dfnsPrivateKey,
    }),
  });
}
