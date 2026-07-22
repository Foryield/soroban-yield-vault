//! Clients des venues externes (aggregator Soroswap, router Aquarius).
//!
//! Chaque sous-module replique les types externes A L'IDENTIQUE (noms de
//! types ET de champs : l'encodage `contracttype` en depend) depuis la source
//! citee en tete de fichier, et expose `attempt` : construit l'appel, invoque
//! la variante `try_` du client, rend `false` sur toute `Err`. Aucune panique
//! imputable a la venue ne traverse `attempt`. Le succes d'un swap est juge
//! PAR LE ROUTEUR sur delta de solde, jamais sur la valeur de retour de la
//! venue.

pub mod aqua;
pub mod soroswap;
