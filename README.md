<p align="center"><img src="assets/logo.svg" width="96" alt="FastForward Engine"></p>

<h1 align="center">FastForward Engine — Game Compatibility Database</h1>

<p align="center">Signed compatibility profiles for <b>FastForward Engine</b>, the speed-control utility for offline, single-player PC games.</p>

---

## What is this repository?

This is the **official, cryptographically signed database** the FastForward
Engine app downloads at startup to know how each game behaves under
time-scaling:

| Status | Meaning |
|---|---|
| 🟢 VERIFIED | Tested: full speed range, audio stays in sync |
| 🟡 PARTIAL | Works with safe defaults (speed cap, audio fail-safe) |
| 🔴 BLOCKED | Refused by design — anti-cheat protected or unsupported |

Every database update is **signed with minisign (Ed25519)**. The app verifies
each download against the public key compiled into the application — a
compromised mirror can never mark a protected title as safe.

## Verify a database yourself

```sh
minisign -Vm profiles.json -P RWQUsGlSOybOiUMHNYIXYBldcfT5rXO2j0mVmOtmmSG9lB2GmoBld57J
```

## Contribute compatibility reports

Played a game with FastForward Engine? Help the community — open an issue
using the **Compatibility Report** template with what worked (speed range,
audio behavior, cutscene handling). Reports are aggregated automatically;
profiles with strong, consistent evidence get promoted after review.

Rules: **offline single-player games only**. Titles protected by anti-cheat
systems (EasyAntiCheat, BattlEye, Vanguard, Ricochet) are permanently
blocked — never reported as compatible.

## Distribution

- App: [FastForward Engine on Steam](https://store.steampowered.com) *(link live at launch)*
- Database (this repo, served via GitHub Pages): `profiles.json` + `profiles.json.minisig`

## License

Database contents: [CC BY 4.0](LICENSE) — attribution "FastForward Engine".
FastForward Engine is a trademark of its project. Not affiliated with any
game publisher or platform.
