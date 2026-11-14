<p align="center">
  <img src="crates/nc_launcher/assets/hicolor/128.png" width="128" alt="Nier Coop" />
  <h1 align="center">Nier Coop</h1>
  <h6 align="center">Online co-op for NieR Replicant ver.1.22474487139 on PC.</h6>
</p>

<p align="center">
  <img src="docs/media/two-players.gif" width="720" alt="Two players in the same world" />
</p>

---

## About
Play _NieR Replicant ver.1.22474487139_ in co-op with another person.

This is a fan project. It is not affiliated with or endorsed by Square Enix.

## Status
Early. A full play-through isn't possible yet.

**Mostly tested on Linux through Proton.**

### What's Synced
* Movement, jumping, attacking, and SFX
* NPC locations and animations
* Companions, which follow the host
* Combat and most sealed verses
* Enemy projectiles
* Boss fights, which the host starts (4/27)
* Downed and revive
* Aggro of enemies
* Cutscenes, which are vote-skipped
* Moving between areas
* Doors
* Lifts, which either player can call
* Bombs, and the walls they break
* Minecart rides, which the host starts
* Story progress, which follows the host
* Checkpoint rewind, together
* Dialog, which can be entered midway
* Story choices, which the host makes
* Chests and item spots: both players get the items. Gathering spots are per player
* Each player has their own inventory and gold. Key items are shared
* Weapons
* Music
* Weather and time
* Names over players and a party frame

### Bonuses
* Skip starting movies and the main menu. The launcher handles everything for you, including saves
* Migrate an existing save to be a "co-op save"
* Either player can pick the prologue character, and the guest can play as Kainé. The choice is made at campaign start
* The game will no longer pause in the pause menu

### Warning
Story progress and saves are volatile. There are thousands of flags that must be synced and can fall out of order. Your normal save is never touched; co-op uses its own save file.

A list of bugs that may likely happen:
* Cutscenes can sometimes start early or late, and may not be perfectly synced
* Enemies on the guest will slide after being struck
* Rides, mini-games, puzzle rooms and fixed-camera rooms run separately for each player

## Getting started
1. Download the launcher from the GitHub Releases page and run it.
2. Settings: press Find to locate your game, then Install mod.
3. New game: type your name and pick your character. One player presses Host a lobby and sends the ticket, the other
   pastes it and presses Join a lobby. When both are ready, the host presses Start.
4. Continue: each player picks a co-op save slot, then host and join the same way.
5. Insert opens the in-game menu.

When something goes wrong, the footer has Copy log and Export logs, to send with a bug report.

## Building
You need the Rust toolchain pinned in `rust-toolchain.toml`. The hook is a Windows DLL (`x86_64-pc-windows-msvc`);
on Linux, build it with `cargo xwin`. The launcher builds on both Windows and Linux.

## Contributing
Before you open a pull request, run these from the repository root:

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
