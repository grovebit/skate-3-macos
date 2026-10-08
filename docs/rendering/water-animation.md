# Water animation

How the original game animates its water: the PCA ocean table, the update
that steps through it, and the clock the water and scrolling shaders read.
The port's code is `WaterAnimation` in
[`retail_render.rs`](../../crates/skate-game/src/render/retail_render.rs) and the
extractor [`ocean_pca.py`](../../tools/asset_pipeline/ocean_pca.py).

Evidence is from the owned **base-disc** `default.xex`, SHA-256
`1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f`, through
its mapped image (base `82000000`, SHA-256
`ce1e3ae512ee08bb716529be671ee112c664414ce9541f14b84f5e5791f13f42`), read by
static disassembly. Shader evidence is the Xenos microcode and constant tables
of the owned `water_defaultPS`, `flowingwater_defaultPS`, `ocean_defaultPS`
and `scrollincandescent_defaultVS` objects. The upstream port's Title Update 3
addresses (Init TU3 `827905B0`, update TU3 `82790858`, table TU3
`830118D8`/`83011A40`) were leads only; every address below was confirmed on
the base disc.

## The PCA table

`cPCAWaterAnimationData::Init` (`8276D120`) allocates a table object tagged
`PCAWeights` and points it at 30 frames (`8276D1C4`, stored at `8276D288`):

- means at `82FC3978`, three floats per frame;
- weights at `82FC3AE0`, 24 floats per frame: three elements (R, G, B) of
  eight weights each.

The means and weights are one contiguous table of 3,240 bytes, SHA-256
`6febf907c06ae35dc1d7d6b366bd0fab56783cb52bcdc8bc8dd67163136a162a`.

Init registers seven `vec4` shader constants by FNV-1 name hash. The names
hash exactly to the registered keys:

| Object offset | Constant | Hash |
| --- | --- | --- |
| +00 | `g_fPcaMean` | `4CA839B2` |
| +10, +20 | `g_fPcaWeightsR_0`, `R_1` | `4D67B5DF`, `4D67B5DE` |
| +30, +40 | `g_fPcaWeightsG_0`, `G_1` | `4D8809AA`, `4D8809AB` |
| +50, +60 | `g_fPcaWeightsB_0`, `B_1` | `0D8FF3AF`, `0D8FF3AE` |

It also sets the frame counter (+140) to 0, a second counter (+142) to 1, and
the remainder (+144) and blend (+148) to 0.

## The update

`8276D3C8` runs with the frame's `dt` in f1:

1. `remainder += dt`. If `remainder > 1/30` (`82325FAC`, strictly greater),
   it subtracts 1/30 and increments both 16-bit counters. It steps at most one
   frame per call, so below 30 calls a second the animation slows down.
2. The frame is counter +140 modulo 30 (`divw` at `8276D4A0`).
3. It writes that frame's mean times 1/255 (`8213F07C`, `fmuls`) into
   `g_fPcaMean`, with 1.0 (`82314D90`) as the fourth component, and element
   e's eight weights times 1/255 into +10+20e and +20+20e. The constants are
   therefore R, G, B in the order the table stores them.

Two other paths in the routine are not ported because nothing uses them:

- If the table pointer (+150) is null, it interpolates a second source (+14C)
  between counters +140 and +142 with the blend +148 (`8275B9C0`). Only Init
  sets +150 (`8276D190`). +14C is only ever cleared, by the constructor
  (`8276DBD4`) and the teardown (`8276D0D4`).
- A global (`830723BC`) gains 1/60 (`82084998`) per call and resets to 0 above
  5.0 (`821EE560`). The routine writes it to +130.w, and +130.xyz from three
  attributes and the remainder. +130 is not a registered shader constant, and
  no code reads it apart from a bulk copy of the render state (`8276C8C8`).
  This is the clock the upstream port used for water time (0.5 units a second
  at 30 updates, looping every 10 s). The water shaders do not read it.

## The world-animation clock

`8276E198` is the update's only caller (`8276E394`). World update `8277AF90`
calls it once per presentation update at `8277B30C`, with no branch that
skips it. Presentation update `826B9860` calls the world update at
`826B9B48`. It:

1. Reads the presentation clock, `*(*(83027DA0) +64)` virtual +10 then +4, as
   whole ticks (an unsigned integer) and a fraction.
2. Subtracts the previous reading's ticks, held as a float (`fsubs`). If the
   difference is strictly between -15 and 15 (`822F4120`, `820BCF8C`),
   `dt = |(fraction - previous fraction) + difference| × 1/60`
   (`8276E294..8276E2B4`). Otherwise `dt` is 0.
3. Adds `dt` to `g_fAnimationTime.x` (FNV-1 `E5A76988`, render manager
   `*(83027DC8) +443D0`, registered by `826BEF00`) at `8276E2F0..8276E32C`.
   It sets `.y` from an attribute (class `CC8734BAB1D5BFB0`, field
   `A5EBB314832D0BBB`, default 0).
4. Calls the update with the PCA object and `dt`.
5. Stores the reading as the previous one (`8276E398..8276E3B4`), whether or
   not the change counted.

When `*(*(83073564) +4) +C0` is set or `827FBA00` returns true,
`8276E1E8..8276E24C` takes `dt` from another time source instead, probably
replay time. That source is not traced.

`g_fAnimationTime.x` is therefore the sum of `dt` in seconds. It does not
wrap. The only per-frame writer of it found is `8276E198`. `8276C030` and
`8276C700` save and restore the whole constant block around a pass.

## Lifetime and cadence

- The manager constructor `8276DB10` (from the world-render constructor
  `827797E8` at `82779D50`) clears both table pointers, runs Init and seeds
  the previous reading with -15 ticks and fraction 0 (`8276DC68..8276DC80`),
  so its first update adds nothing. The teardown is `8276D098`.
- `826B8620` deletes and rebuilds the world-render object, which reruns the
  constructor. That resets the PCA counters, the remainder and the previous
  reading, but not `g_fAnimationTime`, which lives in the render manager.
- Presentation update advances the presentation clock by the frame time only
  when its state (+44) is 18, a virtual check on the clock holder passes and
  flag 10 of the object at `*(83073544)` is clear (`826B99DC..826B9AEC`).
  What those states mean was not traced; the port assumes the clock stops
  while the game is paused.

## What the shaders read

| Shader (port family) | Time | Use |
| --- | --- | --- |
| `water_defaultPS` (33) | c22 = `g_fAnimationTime` | 12: `mul r3, c19, c22.x` (c19 = `m_params[1]`); 22–46: R, G, B = `i_normal`·`_0` + `i_normal2`·`_1` + mean x, y, z |
| `flowingwater_defaultPS` (30) | c15 | 8: `mul r6, c12, c15.x` (c12 = `m_params[1]`) |
| `scrollincandescent_defaultVS` (14) | c9 | 10: `mul r1.x, c9.x, c7.x` (`i_uAnimationSpeed`) |
| `ocean_defaultPS` (31) | none | 13–24: the same sums in R, B, G order against the mean's x, z, y |

`treeanimate_defaultVS` also reads `g_fAnimationTime.x` and `.y`; tree
animation is not ported.

## The port

- **Extraction.** Setup's environment group runs `ocean_pca.py`. It decodes
  the owned `default.xex` with the `xex2` package (through
  `tools/owned_game/xex.py`, shared with the collision mix export and the
  native verifiers), checks the executable, its decoded image and the table
  against the verified hashes, and writes
  `assets/private/ocean-pca.json`: 30 frames of seven rows in the constant
  order above, each value rounded to single precision as `fmuls` does. If it
  fails, setup records `environment-status/ocean-availability.json`, and the
  ocean and still-water families render as family 1.
- **Renderer.** `WaterAnimation` reproduces `8276E198` and the update's PCA
  step. `FrameState.clock.x` is `g_fAnimationTime.x`, and `FrameState.pca` is
  the selected frame in native order. Families 14, 30 and 33 multiply
  `clock.x` by their authored speeds. The world material log names each
  unsupported shader and family.

Host adaptations and provisional parts:

- Bevy's virtual clock stands in for the presentation clock. It pauses during
  map transitions and while the single-player menu is open; the multiplayer
  menu does not pause it. It is read as 60 ticks a second plus a fraction. That
  rate is inferred from the 1/60 s per tick the update applies, its 15-tick
  guard and the 1/30 s frame period. The clock's own class was not identified.
- The update runs once per rendered frame, standing in for once per
  presentation update.
- The state lasts as long as the app. Which of the port's map switches
  correspond to the original's world-render rebuild is untraced, so map
  switches keep it.
- Not ported: the replay time source, `g_fAnimationTime.y`, the unused +130
  vector and the unused interpolating source.

## Validation

- Unit tests: `tools/asset_pipeline/test_ocean_pca.py` checks the table
  layout, the constant order and the single-precision scaling on synthetic
  images. The `water_clock_*` tests in `retail_render.rs` check the first
  update, the strict 1/30 boundary, the ±15-tick guard, the absolute value,
  one step per update, the 16-bit counter wrap and the modulo 30, and the rate
  at 20 to 240 updates a second.
- Extraction from the owned executable gives 30 frames, and the table hash also
  matches the mapped image the research tools read.
- University: the log falls from 52 to 26 materials rendered as family 1, and
  the remaining 26 are family 0 (10 `advertisement.default`, 15
  `environment.transparent`, 1 `model_default`). Without the table the
  reservoir (25 `ocean.default` materials) renders black. With it the
  reservoir shows blue-grey rippled water, and the Chan Center fountain pool,
  previously black and speckled, shows grey water with highlights.

Not yet compared with the running original: the animation's speed and look
in motion, and the clock's behaviour across pauses, replays and map loads.
