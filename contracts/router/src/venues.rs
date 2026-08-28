//! Client de la venue externe (router Aquarius).
//!
//! Le sous-module replique les types externes A L'IDENTIQUE (noms de types ET
//! de champs : l'encodage `contracttype` en depend) depuis la source citee en
//! tete de fichier, et expose `attempt` : construit l'appel, invoque la
//! variante `try_` du client, rend `false` sur toute `Err`. Aucune panique
//! imputable a la venue ne traverse `attempt`. Le succes d'un swap est juge
//! PAR LE ROUTEUR sur delta de solde, jamais sur la valeur de retour de la
//! venue : une venue qui ment sur ce qu'elle a servi ne trompe pas le routeur.
//!
//! La venue Soroswap a ete RETIREE le 28/08/2026 (compromission, cf.
//! docs/plans/2026-08-28-retrait-soroswap-aquarius-seul.md). Le contrat
//! `attempt` rendant `false` reste inchange : le routeur mono-venue le traduit
//! desormais en panique `VenueFailed` au lieu d'un passage au secours.

pub mod aqua;
pub mod convert;
