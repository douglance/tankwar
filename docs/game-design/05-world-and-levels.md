# World and level design brief

Audience: level designers, environment artists, historical researchers and simulation engineers. Purpose: define the first playable battlefield and how future arenas are accepted. Status: design brief; no complete arena has been acquired or built.

## World intent

Build a believable WWII combat environment presented with low-poly geometry, subdued colors and readable silhouettes. The world is physically navigable and supports cover, concealment, elevation and long sight lines. Historical context comes from the chosen theater, period architecture and source-backed vehicle configurations; avoid presenting an invented layout as a documented battle.

## Proposed first arena: rural village and outskirts

The first map is a fictional European rural settlement and surrounding agricultural land. Its exact theater, season, year and faction mix are unresolved research decisions; do not label assets historically accurate until checked. The arena must support 1v1 and 2v2, with a fair route from each spawn to meaningful cover and positions. Provide a village center, a road corridor, open fields, a tree line, gently varied terrain, stone or masonry walls, ruined structures and scattered debris. Roads connect the settlement to outskirts; avoid modern signs, paint and street furniture unless verified for the scenario.

```text
+---------------+     +-------------------+     +---------------+
| West approach | --> | Village and cover | <-- | East approach |
+-------+-------+     +---------+---------+     +-------+-------+
        |                       |                       |
        +-----------------------+-----------------------+
                    fields / tree line
```

This diagram shows relationships, not a final map. Choose dimensions, spawn spacing, cover density and engagement ranges only after source and gameplay tests; no real-world scale is established yet. One map contains paths for maneuver without making any spawn immune to approach. Open ground offers visibility at the cost of exposure; buildings and vegetation alter sight and movement according to their actual collision and visibility data.

## Environment behavior

Define terrain height and traversability, slopes, collision and occlusion consistently for simulation, agent observations and the spectator view. Each obstacle has a material, collision shape and explicit destruction behavior. Trees may conceal or obstruct only as implemented and tested; an attractive transparent canopy is not automatic proof of concealment. Debris, ruins and buildings must have plausible scale for the selected vehicles. Weather, time of day, fog, destructible buildings and terrain deformation remain optional until their data, performance and replay behavior are specified.

Initial ranked scenario has fixed spawns, no depot and a 15-minute limit. If later scenarios add depots, specify locations, ownership, stock, access, contesting, interruption and refill rates, and verify the battlefield stays playable in both modes.

## Level production checklist

1. Select the theater, season and timeframe using historical references; document what is fictional.
2. Assemble licensed terrain, vegetation, roads, buildings, ruins, debris, sky and audio; resolve any missing source or rights.
3. Produce a scale-consistent blockout and traversability/occlusion maps; inspect every spawn and intended approach.
4. Build the final environment and compare actual sight images with spectator geometry; test collisions and known blocked views.
5. Playtest repeated mirrored 1v1 and 2v2 matches, profile simulation and rendering, and version the shipped map and manifest.

Acceptance requires one complete rights-reviewed arena; plausible visual and vehicle scale; no unreachable or trapped spawn; no collision or visibility leaks; readable sight lines; reproducible map version in replay; and recorded balance/playability findings. No completion claim exists before those artifacts and tests exist.
