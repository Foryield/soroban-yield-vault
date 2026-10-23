// @ts-check
// --- Lecture des events Soroban au RPC ---------------------------------------
// Module JavaScript pur, partage par le script d'ingestion (Node, tache
// quotidienne) et par le dashboard (navigateur) : une seule pagination.
//
// Le RPC ne garde les events qu'environ 7 jours (D6a, section 6) et ne
// parcourt qu'une tranche de ledgers par requete : une reponse vide n'est pas
// une fin, seul le curseur dit jusqu'ou il a lu. On pagine donc par curseur
// jusqu'au dernier ledger ferme.
//
// Les events sont gardes BRUTS (topics et valeur en XDR base64) : c'est la
// forme que la chaine a emise, la seule qui ne depende d'aucun decodeur.

/**
 * @typedef {{
 *   id: string,
 *   contract: string,
 *   ledger: number,
 *   closedAt: string,
 *   txHash: string,
 *   topics: string[],
 *   value: string,
 * }} RawEvent
 */

/**
 * @typedef {{
 *   events: RawEvent[],
 *   startLedger: number,
 *   latestLedger: number,
 *   oldestLedger: number,
 * }} EventPage
 */

const PAGE = 100;

/** Ledger d'un curseur RPC : TOID dont les 32 bits hauts portent le ledger. */
function cursorLedger(/** @type {string} */ cursor) {
  return Number(BigInt(cursor.split("-")[0]) >> 32n);
}

/**
 * Events des contrats depuis `fromLedger` (inclus), ramene au plus ancien
 * ledger que le RPC garde encore. `startLedger` dit ou la lecture a reellement
 * commence : s'il depasse `fromLedger`, des ledgers sont sortis de la fenetre
 * sans avoir ete lus.
 *
 * @param {import("@stellar/stellar-sdk").rpc.Server} server
 * @param {string[]} contractIds au plus 5, limite d'un filtre RPC
 * @param {number} fromLedger
 * @returns {Promise<EventPage>}
 */
export async function fetchContractEvents(server, contractIds, fromLedger) {
  const health = await server.getHealth();
  const startLedger = Math.max(fromLedger, health.oldestLedger);
  const filters = [{ type: /** @type {const} */ ("contract"), contractIds }];
  /** @type {RawEvent[]} */
  const events = [];
  let latestLedger = health.latestLedger;

  if (startLedger > latestLedger) {
    return { events, startLedger, latestLedger, oldestLedger: health.oldestLedger };
  }

  let response = await server._getEvents({ startLedger, filters, limit: PAGE });
  for (;;) {
    latestLedger = response.latestLedger;
    for (const e of response.events) {
      events.push({
        id: e.id,
        contract: e.contractId,
        ledger: e.ledger,
        closedAt: e.ledgerClosedAt,
        txHash: e.txHash,
        topics: e.topic ?? [],
        value: e.value,
      });
    }
    const done =
      response.events.length < PAGE && cursorLedger(response.cursor) >= latestLedger;
    if (done) break;
    response = await server._getEvents({ cursor: response.cursor, filters, limit: PAGE });
  }

  return { events, startLedger, latestLedger, oldestLedger: health.oldestLedger };
}
