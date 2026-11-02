# Asset inventory

This inventory is partial. Source archives now include tank exteriors and environment candidates; they do not constitute a playable arena.

| Asset | Use | Creator | License | Status |
| --- | --- | --- | --- | --- |
| [Brown Mud Leaves 01](https://polyhaven.com/a/brown_mud_leaves_01) | Candidate forest ground material | Rob Tuytel | [CC0](https://polyhaven.com/license) | See local manifest for downloaded maps and SHA-256 hashes; acquisition 2026-11-02 UTC. |

The material is a surface, not terrain geometry. Check its historical and visual suitability in the final arena. Individual file provenance is recorded in its manifest.

## User-favored tank reference

[Lowpoly Tank Pack 01](https://isolatedsoulstudio.itch.io/lowpoly-tank-pack-01) by IsolatedSoulStudio: the creator lists IS-2, KV-2, Panzer IV, M4 Sherman, Tiger II and T-34-76, each with separated body, turret and barrel; FBX and DAE, 1024-pixel color map. The Panzer IV is a free sample; the complete pack requires at least $5. The page does not state reuse or redistribution terms; a commenter asks about the license without a visible answer. The user supplied the Panzer IV archive; a local copy is retained as a reference. Treat its reuse and redistribution rights as unverified. It does not establish interiors, detailed controls, historical accuracy, or full roster coverage. The page metadata also names IS-3 rather than IS-2, so the paid pack contents need direct verification. Accessed 2026-11-02 UTC.

## Additional candidates

| Source | Contents | Terms and status |
| --- | --- | --- |
| [Newc42: Low Poly German WWII Tanks](https://newc-42.itch.io/german-low-poly-wwii-tanks) | Leichttraktor, Panzer III J, Tiger I; separate wheels, turrets and guns; Blender and FBX | Creator states CC0. FBX and Blender source archives downloaded; not visually compared in a shared scene. The Leichttraktor's wartime eligibility is unverified. |
| [MetaWorldOS WWII Tank Pack](https://metaworldos.itch.io/wwii-tank-pack-glb-models) | Panther, Panzer IV, Sherman, IS-2, modern generic vehicle and battlefield props; GLB | Pack page says CC0, but the provider's [asset catalog](https://metaworldos.com/api/game-assets?project=tank-battle&limit=100&page=1) labels listed tank-battle assets proprietary. Resolve provenance and scope before use. Single-mesh models need articulation work. Not downloaded or reviewed. |
| [M4A2 Sherman by barking_dogo](https://sketchfab.com/3d-models/m4a2-sherman-b1fe6a97c7fb454f9ba3e4570568f892) | WWII Sherman exterior model | Page lists CC Attribution; exact license version, format, interior, rigging and fit require verification. Not downloaded. |
| [BlendSwap WWII tank listings](https://blendswap.com/3d/ww2-tank) | Candidate Tiger II, Panzer IV H and T-34 plus others | Listings show varying licenses. Verify each asset page, creator rights, download, and content; not downloaded. |
| [Kenney Impact Sounds](https://kenney.nl/assets/impact-sounds) | 130 general impact sounds | Page lists CC0; direct request returned HTTP 403. Not downloaded; no claim of period-specific recordings. |

A complete, usable arena, tank interiors, sights, vehicle sounds and all wartime variants remain missing. All candidates still require suitability and historical review.

## Low-poly environment candidates

| Candidate | Intended use | License and acquisition |
| --- | --- | --- |
| [Kenney Nature Kit](https://kenney.nl/assets/nature-kit) | Trees, rocks, vegetation | CC0; downloaded from the creator's OpenGameArt listing. Visual and scale fit unverified. |
| [Kenney City Kit Roads](https://kenney.nl/assets/city-kit-roads) | Modular roads | CC0; downloaded from the creator's OpenGameArt listing; modern markings, barriers and signs need period review. |
| [Kenney Modular Buildings](https://kenney.nl/assets/modular-buildings) | Buildings and construction pieces | CC0; not downloaded; period fit and damage variants unverified. |
| [Quaternius Ultimate Modular Ruins](https://quaternius.com/packs/ultimatemodularruins.html) | Broken walls, rubble and modular ruins | CC0; publisher's download folder was inaccessible from the web tool; medieval/fantasy pieces need visual and period review. |
| [Majadroid Apocalyptic Buildings](https://majadroid.itch.io/3d-apocalyptic-building-city-cc0) | Ruined buildings and wreckage | CC0; downloaded from the creator's OpenGameArt listing; skyscraper styling may not fit the chosen theater. |

The alternative model mirror failed TLS certificate verification and its storage endpoint returned HTTP 403. No models were downloaded from it. The environment remains incomplete: a playable battlefield, terrain geometry, period-appropriate ruins, roads and debris still require acquisition and review.

## Acquired archives and integrity

Acquired 2026-11-02 UTC. The archives are unmodified and passed ZIP integrity checks. Scene assembly, scale, visual matching and historical suitability have not been validated.

| Local archive | Source | SHA-256 |
| --- | --- | --- |
| `source-archives/reference/PanzerIV.zip` | User-provided; [creator page](https://isolatedsoulstudio.itch.io/lowpoly-tank-pack-01); rights unverified | `9fee81aa6eacc68966986f211c5a240105e257c76621dced01f19a8213cdd71b` |
| `source-archives/newc42/tanks-fbx.zip` | [Newc42](https://newc-42.itch.io/german-low-poly-wwii-tanks); creator states CC0 | `39590a9f1ecf06a4fc03810e5a5f7e1e90861ea632622d0e29b5d1fe62177604` |
| `source-archives/newc42/tanks-blender.zip` | Same pack, Blender source | `aca2117929ff8de3d2ce23127ea6a11122451ed9a02f3feb6b9ec4296f558c89` |
| `source-archives/kenney/nature-kit.zip` | [Kenney on OpenGameArt](https://opengameart.org/content/nature-kit); CC0 | `fa7974a0d342bfe63c38664ba9f8ec1a4aab8ea25f099bdc56870e33588c4d9d` |
| `source-archives/kenney/city-kit-roads.zip` | [Kenney on OpenGameArt](https://opengameart.org/content/city-kit-roads); CC0 | `374a4552115cbd11b4ec5c930ee31a533562dc78a09af249a655d03f7ea3b959` |
| `source-archives/kenney/city-kit-industrial.zip` | [Kenney on OpenGameArt](https://opengameart.org/content/city-kit-industrial); CC0 | `5b381164e5760f3830a2dbee43b972deee38b2a695d091b56e238ab2910c96d2` |
| `source-archives/majadroid/apocalyptic-buildings.zip` | [Maik Hoffmann / Majadroid](https://opengameart.org/content/3d-apocalyptic-building-city-cc0); CC0 | `4117bd1d2e815223762c1dbd53e80ee050f64ed7e2b4b43a2bc00ebf20b7df6b` |

The user-provided tank contains separate FBX and DAE hull, turret and barrel files and a flat-color texture atlas, but no license file. A quick FBX import did not automatically assemble the turret and gun or apply the atlas; import setup requires work. The ruins pack includes modern skyscrapers and billboards, so only individually reviewed pieces should be considered for a WWII scene. Kenney road markings and industrial buildings likewise need period review. These are candidates, not a visually approved set.

Still needed: a complete arena and terrain, historically suitable ruins and debris, interiors and optics, effects and sounds, and a comprehensive wartime tank roster. Additional sourcing, original work and historical research will be necessary; no full-accuracy claim is established by these models.
