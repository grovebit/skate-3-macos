# Game conversion time in `./play.sh`, 2026-10-08

Integration branch `main` at `37c3205`. Apple M3 Max (16 cores, 48 GB). The
repository and the extracted game folder were on an external NVMe SSD in a USB
enclosure (RTL9210B). On that drive cold reads ran at about 320 MB/s, but
sustained writes ran at only 80 MB/s. The internal SSD wrote at 3.9 GB/s. The
disc image was on the internal SSD. Other applications kept about half the CPU
busy during every run.

## Method

`tools/prepare_assets.py` ran with the arguments `play.sh` passes. It wrote a
fresh output folder with the skating audio included, and each output line was
timestamped. A repeat `./play.sh` with current data does not convert, and is
unchanged. It spends about 1 s before the game starts: 0.1 s on the version
check, 0.2–0.8 s on the no-op `cargo build` and 0.04 s on the dependency check.

## Findings

- The baseline used 276 s of CPU in 340 s of wall time, so it averaged under
  one of the 16 cores.
- Map conversion was bound by writes to the data drive. Three concurrent map
  jobs wrote about 5 GB of intermediate files (extracted archives, raw texture
  caches, mesh arrays) beside the output. Extracting DownTown's 804 MB archive
  took 56 s at 15 MB/s, against 2 s from cache. Converting DownTown alone took
  64 s with its intermediate files on the external drive and 44 s with them on
  the internal SSD.
- The character customiser built its 480 clothing models and 41 pro skaters
  on one core. Rendering roster thumbnails took 56% of the roster's time and
  PNG encoding most of the rest. The roster also left 400 MB of work files in
  every customiser set.
- Skating audio ran 1,761 `vgmstream-cli` processes one after another.
- Output receipts included the `.DS_Store` files Finder writes into folders
  it shows. When Finder rewrote one, the next refresh treated that output
  group as damaged. In a test refresh this rebuilt the clothing library for
  247 s. A receipted `.DS_Store` in the core group would rebuild every group.
- Superseded customiser generations were never deleted. Each one kept about
  2.3 GB and about 10,000 files, and every refresh hard-linked all of them
  into the new installation.

## Changes

- Intermediate files go to the system temporary folder when the startup volume
  has 16 GB free, plus the disc's size for an `.iso`. Otherwise they go beside
  `data/` as before. Peak use was 5.0 GiB for a game folder and 11.2 GiB for a
  disc image, which is now extracted there too.
- The clothing library builds model GLBs and the texture images it needs in a
  process pool. It then indexes them in catalog order, as before. The roster
  builds its characters in a process pool, with its work files in the
  temporary folder.
- Skating-audio clips decode concurrently.
- A refresh that rebuilds maps no longer copies the previous maps first. A
  failed district still keeps its validated old map, including its irradiance
  file.
- Receipts leave out `.DS_Store`, and validation skips it in older receipts.
- Once a customiser generation is current, setup deletes the other generations
  and any unfinished one. The game reads only the current generation, and the
  saved outfit names asset IDs, not generation paths.

## Results

One full conversion each:

| Stage | `main` | Optimized |
| --- | ---: | ---: |
| Maps (3 workers) | 156 s | 49 s |
| Character customiser | 111 s | 43 s |
| Skating audio | 53 s | 12 s |
| Everything else | 20 s | 16 s |
| **Total, game folder** | **340 s** | **120 s** |
| **Total, disc image** | 432 s on 2026-10-07 (86 s extraction) | **133 s** (2.9 s extraction) |

Run alone, the customiser library took 12.6 s, the roster 10.5 s and the audio
6.9 s. In the full run, writing their 1.9 GB of output to the external drive
makes up most of the difference.

A refresh after a maps-only change, from an installation the same code had
built:

| Step | `main` | Optimized |
| --- | ---: | ---: |
| Copy the previous installation | 37 s | 25 s |
| Maps | 228 s | 96 s |
| Validation and customiser checks | 33 s | 38 s |
| Clothing library rebuilt after a `.DS_Store` rewrite | 247 s | – |
| **Total** | **564 s** | **167 s** |

Without the spurious library rebuild, `main` would have taken about 317 s. The
optimized copy step still hard-linked two customiser generations; deleting
superseded ones leaves one.

The outputs are byte-identical to `main`'s:

- core, character and environment data;
- all 8,081 clothing-library files and the library index;
- the 41 roster characters;
- the 1,761 audio clips.

Maps and props have identical geometry, textures and collision. Their metadata
differs only in the absolute path of the temporary folder. That path already
changed between installations, because it contained the random installation
ID.

`pipeline-equivalence.json` keeps existing installations current for the two
fingerprints these changes touch: `maps`, through `map_job.py`, and
`character`, which drops an import-time banner that every pool worker printed.

## Limits

Each configuration was run once on one machine, under background load. A
repository on the internal SSD gains less from the temporary folder, because
its writes were already fast. The refreshes read game archives that were no
longer cached, so their map step took longer than in the full conversions.

Two refresh costs remain:

- Every refresh re-hashes the current customiser generation (about 2 GB) and
  the game archives the customiser reads.
- Reusing an unchanged customiser stage in a new generation re-hashes its
  files and hard-links them one at a time. For the clothing library's 8,084
  files this took 41 s with a warm cache and 152 s with a cold one.
