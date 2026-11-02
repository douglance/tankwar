# Tankwar spectator art direction

- Look: low-poly, flat-shaded, readable silhouettes, restrained earth tones (ochre fields, brown mud, muted greens, terracotta and slate roofs). Distance fog toward a grey-blue sky.
- Arena: the fictional village of Lesmont. It has a church with a walled yard at the center, a dozen cottages along one main road, a crossroad, fields and vineyard rows, a fence line, a tree line on the ridge, and open west and east approaches where the sides start. There are no skyscrapers, billboards or modern road signs.
- Axes: village x east, z south, y up. Feed (x, y) maps to world (x, z). The contract sim plane is assumed to be (x, -y) until the axis request in CONTRACT-REQUESTS.md is answered.
- Tanks: assets/derived/tanks/pz3j.glb (West, Panzer III Ausf. J) and tiger1.glb (East, Tiger I). Both are derived from the Newc42 CC0 archive. The Panzer IV archive is a style reference only. It is not unpacked or shipped.
- Other sources (all CC0): Kenney Nature Kit and Kenney Roads for trees, rocks, fences and road pieces, and the Poly Haven brown mud leaves texture for ground tone.
- Cameras: orbit (the battle midpoint), follow (behind the selected tank), and the crew stations (gunner sight, commander hatch, driver visor). Station views are reconstructions from the spectator feed, not what an agent sees.
- HUD: fuel, ammunition per round type, loaded round, crew, damage, and the command timeline. Chips show LIVE, LAG, DISCONNECTED, RECOVERED, RESULT, ABORTED and MISSING INTERVAL, plus DEMO FEED on every demo match. Command text is escaped: control and bidi characters are removed and markup is shown literally.
- Sound: synthesized engine idle and gun report, spatialized at the tank. It plays only for spectators, is derived from events already public, and is never sent to agents.
