# Mitmachen bei Esslinger Cyberspace

**Alle dürfen mitcoden.** Esslinger Cyberspace (früher Esslinger Mesh) ist Public Domain.

Sven Normen Eßlinger hat entschieden: dieses Repository ist ein öffentliches Commons. Jede Person darf forken, ändern, Pull Requests öffnen und den Inhalt weiterverwenden.

## Lizenz der Beiträge

Der Inhalt dieses Repositories steht unter [CC0 1.0 Universal](https://creativecommons.org/publicdomain/zero/1.0/) (Public Domain), siehe [LICENSE](LICENSE). CC0 wird nicht durch eine engere Lizenz ersetzt.

**Mit einem Pull Request widmest du deinen Beitrag ebenfalls CC0 1.0.** Du verzichtest, soweit das anwendbare Recht das zulässt, auf das Urheberrecht und verwandte Schutzrechte an deinem Beitrag. Wo ein vollständiger Verzicht nicht wirkt, gilt die in CC0 vorgesehene öffentliche Lizenz. So bleibt das Commons Public Domain, und spätere Ansprüche sollen das Weiterverwenden nicht blockieren.

Wenn du einen Beitrag nicht unter CC0 widmen kannst oder willst, öffne dafür bitte keinen Pull Request.

### Ausnahme: `contracts/amm`, `contracts/nft`, `contracts/nft-market`

Diese Ordner haben eine eigene Lizenz und bleiben **Apache-2.0** (siehe die `LICENSE` im jeweiligen Ordner und [README](README.md)). Änderungen in diesen Ordnern stehen unter Apache-2.0. Die CC0-Widmung gilt für den übrigen Inhalt.

## So arbeitest du mit

1. **Fork** dieses Repository auf GitHub.
2. **Clone** deinen Fork lokal.
3. **Branch** anlegen, zum Beispiel `git checkout -b mein-thema`.
4. Änderungen klein und nachvollziehbar halten.
5. Im jeweiligen Contract-Ordner (`contracts/amm`, `contracts/nft`, `contracts/nft-market`): `cargo test` und `cargo clippy --all-targets -- -D warnings` (die Prüfung in `.github/workflows/rust.yml` macht dasselbe).
6. **Pull Request** gegen `main` öffnen. Schreib kurz, was sich ändert und warum.
7. Mit dem PR bestätigst du die CC0-Widmung. Für Dateien unter `contracts/amm`, `contracts/nft` und `contracts/nft-market` gilt Apache-2.0.

## Was hier nicht hingehört

Dieses Repository ist eine lokale, quelloffene Vorbereitung. Es enthält keinen Live-Chain-Zustand.

Bitte nicht hinzufügen:

- Schlüssel, Seeds, Mnemonics, `.env`, Tokens oder Passwörter;
- `genesis.json`, Validator-Material oder Keyrings;
- Join-Cards mit Namen, Adressen oder Identitäten;
- private Pfade, Zugangsdaten oder Live-Endpunkte.

AMM, CW721-Collection und NFT-Escrow sind Prototypen und nicht auditiert. Nicht mit echten Werten deployen, ohne unabhängige Prüfung und ausdrückliche Governance-Freigabe.

## English note

Everyone may co-code. This work is public domain under CC0 1.0 Universal. By opening a pull request you dedicate your contribution to CC0: you waive copyright where the law allows, and where a waiver is not effective the CC0 public license applies, so the commons stays public domain. Do not replace CC0 with a more restrictive license. The folders `contracts/amm`, `contracts/nft` and `contracts/nft-market` stay Apache-2.0. Fork, branch, and open a pull request against `main`. Do not add secrets, genesis files, identity-bearing join cards, or private paths.

Sicherheitsprobleme bitte nicht als öffentliches Issue melden, sondern wie in [SECURITY.md](SECURITY.md) beschrieben.
