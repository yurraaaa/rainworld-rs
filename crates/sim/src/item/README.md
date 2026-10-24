# `item/`

`weapon.rs` (`Weapon`, `WeaponKind::{Rock, Spear}`), `spear_stick.rs` (`SpearStick`, `SpearStickKind`),
`on_back_stick.rs` (`OnBackStick`, the Hunter's `AbstractOnBackStick`),
`tests/`.

| File | Holds (key types) | Ports |
|---|---|---|
| `hits_turn.rs` | thrown rock/spear vs creatures | `Weapon.Update` Thrown branch |
| `turn.rs` | slugcat grabbing: `Grabability` (dead lizards: `Drag`), `CanIPickThisUp`, pick-up candidate | `Player` item rules |
| `weapon_turn.rs` | rocks/spears in the update list, loops, sparks | - |
| `weapon_world.rs` | the world's realized rocks/spears per room, `ShortCutsReady`, the collision pass, `Abstractize` | `Room.Update`, `AbstractRoom.Abstractize`, `Weapon` |
| `mod.rs` | abstract objects (`AbstractObject`, `Entity`, `ObjectData`, `AbstractSpear`), save strings | `AbstractPhysicalObject` |
