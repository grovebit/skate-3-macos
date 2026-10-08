# Mods

`./play.sh` loads every mod package in this folder (it sets `SKATE3_MODS` to
it). A package is a folder or `.zip` with a `mod.json` and its Lua entry file;
see the [mod SDK](../sdk/README.md) to write one. Enable, disable and configure
mods in game under Extras > Mods.

- [`Skyline_Drive_Mod`](Skyline_Drive_Mod/README.md): a drivable Skyline with
  solid deformation, using the same package id as the SDK's `skyline` example,
  so enable only one of them.

Other launches look for a `mods` folder beside the game binary unless
`SKATE3_MODS` names another folder.
