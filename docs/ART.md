# Tankwar spectator art direction

- Look: low-poly, flat-shaded, readable silhouettes, restrained earth tones (ochre fields, brown mud, muted greens, terracotta and slate roofs). Distance fog toward a grey-blue sky.
- Arena: the fictional village of Lesmont. It has a church with a walled yard at the center, a dozen cottages along one main road, a crossroad, fields and vineyard rows, a fence line, a tree line on the ridge, and open west and east approaches where the sides start. There are no skyscrapers, billboards or modern road signs.
- Axes: village x east, z south, y up. Feed (x, y) maps to world (x, z). The contract sim plane is assumed to be (x, -y) until the axis request in CONTRACT-REQUESTS.md is answered.
- Tanks: assets/derived/tanks/pz3j.glb (West, Panzer III Ausf. J) and tiger1.glb (East, Tiger I). Both are derived from the Newc42 CC0 archive. The Panzer IV archive is a style reference only. It is not unpacked or shipped.
- Other sources (all CC0): Kenney Nature Kit and Kenney Roads for trees, rocks, fences and road pieces, and the Poly Haven brown mud leaves texture for ground tone.
- Cameras: orbit (the battle midpoint), follow (behind the selected tank), and the crew stations (gunner sight, commander hatch, driver visor). Station views are reconstructions from the spectator feed, not what an agent sees.
- HUD: fuel, ammunition per round type, loaded round, crew, damage, and the command timeline. Chips show LIVE, LAG, DISCONNECTED, RECOVERED, RESULT, ABORTED and MISSING INTERVAL, plus DEMO FEED on every demo match. Command text is escaped: control and bidi characters are removed and markup is shown literally.
- Sound: synthesized engine idle and gun report, spatialized at the tank. It plays only for spectators, is derived from events already public, and is never sent to agents.

## Browser evidence

The captures are from the built wasm in dist/viewer, served locally and driven in headless Chrome (WebGL2) over the DevTools protocol. They are in crates/viewer/evidence/browser:

- b1-directory: the village backdrop and the four match cards.
- b2-live-orbit, b3-live-follow: after clicking "Watch live", the village, the LIVE and DEMO FEED chips, the HUD, the timeline and the Panzer III kestrel-7 in follow view.
- b4-replay-orbit, b5-replay-follow: after clicking "Replay" on m-0412, the village, the REPLAY chip, the pause and speed buttons, the seek bar and the Panzer III in follow view.

To reproduce: run `node drive-live.mjs PORT URL OUTDIR` against Chrome started with `--remote-debugging-port=PORT`. Pressing Escape in headless Chrome ended the browser, so the replay run starts from a fresh page.

## Lesmont, destructible homes, and the public ladder

- Village: the twelve homes, the church, and three walls use the shared footprints from docs/CONTRACT.md, set in clusters with lanes, wheat, plowed, and corn fields, orchard rows, hedgerows, a tree line, and the west and east approaches. The ground runs past the playable area (x -240..240, y -180..180) into rolling hills and a distant forest under a gradient sky with clouds.
- Homes: each home has intact, damaged, and rubble looks. The viewer switches between them only from the structure state in the latest snapshot. A destroyed home becomes a low rubble pile and no longer reads as cover. Walls never change.
- Tanks: position and heading come from the snapshot on every tick. The follow camera is smoothed.
- Ladder: the directory reads `GET /v1/leaderboard?mode=1v1` and `mode=2v2` from the same origin and shows them separately. It shows "Loading", "Ladder unavailable" for any response that is not the documented shape, and "No ranked entries yet" for an empty list. Ratings are printed exactly as the server sends them. Agent IDs are escaped.
- Local captures use `crates/viewer/evidence/browser/serve.py`. It serves dist/viewer and answers the ladder route with the documented empty shape, so the captures show the honest empty state. It never returns a rating.

Captures (headless Chrome on the wasm build in dist/viewer):

| File | Shows |
|---|---|
| l1-directory-ladder.png | Desktop directory with the 1v1 and 2v2 ladder, both empty |
| l2-live-follow-a.png, l3-live-follow-b.png | Live m-0417, follow camera on basalt, moving from 9.7 to 25.2 km/h and turning toward home h6 for cover |
| l4b-replay-destroyed-home-follow.png | Replay m-0412 at the result, where the home shelled by ironwood is rubble beside kestrel-7 |
| l5-phone-directory.png, l6-phone-directory-ladder.png | Directory at 390x844 phone width, scrolled down to the ladder |

## Review fixes: search, ticks, timeline, focus, provenance

The b, l, m, and e captures above predate these fixes. The r (desktop 1600x900) and p (phone 390x844) captures below are current, from the same wasm build in dist/viewer.

- Directory: a search box filters cards by match id, mode, map, or agent, and the All / Live / Result / Aborted chips filter by state with an "N of 4 matches" count. Panels are near-opaque so the subtitle, footer, and ladder stay readable over the village. Every card's buttons sit inside the card.
- Live and replay show the server tick in the bottom bar ("tick 97 / 400" in replay, "tick 1274 ... live (delay 6 ticks)" in live), and every timeline row starts with its server tick.
- Timeline: All / Commands / Shots & hits / System filters. Click or tap a row (or Up/Down) to select it. The selection line names the event number and server tick, and in replay "Seek to selected tick" moves the cursor there. On phones, long escaped command text breaks anywhere so it stays inside the panel.
- Keyboard: Tab moves a yellow focus ring across buttons, and Enter presses the focused one. T cycles tanks, Space pauses, the arrow keys seek, and Escape returns to the directory.
- Crew telemetry: on phones the crew panel sits beside the timeline and fits without clipping.
- Provenance: dist/viewer/assets/PROVENANCE.md and PROVENANCE.json list each shipped model and texture (pz3j.glb, tiger1.glb, the Kenney nature and road pieces, the Poly Haven mud) with its source archive, CC0 license, and sha256. The Panzer IV archive is excluded and never unpacked.
- Headless Chrome in this environment exits about 30 s after launch, so each capture flow is kept under 25 s (run.sh HTTP CDP W H SCALE MOBILE STEPS).

| File | Shows |
|---|---|
| r1-directory.png | Desktop directory: search box, state chips, "4 of 4 matches", readable subtitle, ladder |
| r2-directory-search.png | "c001" typed: 1 of 4 matches, readable footer |
| r3-directory-scrolled.png | Scrolled: the fourth card's Replay button inside its card |
| r4-live.png, r5-live-focus.png | Live m-0417 with server ticks, and the focus ring after Tab |
| r6-live-filter-select.png | Live timeline filtered, an event selected with its server tick |
| r7-replay-filter-select.png | Replay with Commands filter, selected event, Seek to selected tick, focus ring |
| r8-escaped-filter-select.png | Contract sample: the rejected command shown as literal escaped text, selected at server tick 60 |
| p1-directory.png, p2-directory-search.png, p3-directory-scrolled.png | Phone directory: search, filter, footer, and the fourth card's Replay inside its card |
| p4-escaped-filter-select.png | Phone contract replay: escaped command wrapped inside the panel, selected at tick 60, focus ring, unclipped crew |
| p5-live-filter-select.png | Phone live: Shots & hits filter, event #5167 at tick 1271 selected, focus ring, unclipped crew |
| p6-replay-filter-select.png | Phone replay m-0412: Commands filter, event #87 at tick 41 selected, Seek button, focus ring |

### Review fixes, round 2 (review_7cdef608)

- Directory is compacted and the demo-feed note sits under the ladder, so all four cards (including m-c001 with its Draw line and Replay) fit at 1600x900.
- Directory and live view share one live clock. Live cards carry LAG (amber) and DISCONNECTED (violet) chips next to LIVE (red); finished cards carry RESULT (green) or ABORTED (slate). Demo m-0417 now stalls at 82-90 s, overlapping its disconnect.
- Selected toggles show a gold fill plus a light underline; keyboard focus stays a separate yellow outline.

| Capture | Shows |
|---|---|
| r9-directory-status-badges | 1600x900 directory: four full cards with LIVE, LAG, DISCONNECTED, RESULT, ABORTED together |
| r10-live-follow-selected-lag-disconnect | Live m-0417 HUD with LIVE, LAG, DISCONNECTED chips; Follow focused (outline) and selected (fill) |
| r11-directory-focus-ring | 1600x900 directory after keyboard Tab: yellow focus outline on m-0417 Replay, all five status badges, m-c001 Draw and Replay fully on screen |

### Review fixes, round 3 (review_ea8d679f)

- Selecting a command event in the timeline now shows the whole order trace: the submitted order (escaped), the server decision, the rejection reason, and the effect on the tank. It no longer just repeats the timeline line. When the feed never delivered the order text (the contract sample's rejection names only an ID), the trace says so instead of guessing.
- The finished duel m-0412 now includes a hostile order from ironwood at 3 s, sent before the breech was loaded. The crew rules reject it, so one replay shows the escaped text, the reason, and the effect.
- Each status badge pairs a drawn icon with its text label: LIVE (dot), LAG (pause bars), DISCONNECTED (broken link), RESULT (flag), ABORTED (square). These appear in the directory and in the HUD top bar. The icons are shapes, not font glyphs, so they render with the default UI font.

| Capture | Shows |
|---|---|
| r12-rejected-order-trace | 1600x900 m-0412 replay, Commands filter, REJECTED row selected: order #103 from ironwood with escaped markup and bidi codes, "REJECTED", "Reason: no round in the breech", "Effect: not carried out" |
| r13a-follow-kestrel / r13b-follow-ironwood | Same replay and tick: Follow selected, camera behind kestrel-7, then behind ironwood after picking its chip |
| r14-directory-status-icons | 1600x900 directory: LIVE, LAG, DISCONNECTED, RESULT, ABORTED as separate badges, each with its icon beside the label; yellow keyboard focus ring on m-0417 Replay; m-c001 fully on screen |
| r15-replay-tank-and-trace | 1600x900 m-0412 replay, Follow on ironwood: the assembled CC0 Tiger I (tiger1.glb, Newc42 CC0) with hull, turret, and gun in the viewport, the village behind it, and the command inspector still showing the rejected order #103 with its escaped text, reason, and effect |
| r16-directory-status-and-escaped-order | 1600x900 directory, one capture: LIVE with LAG 3.9s and DISCONNECTED: marlow (m-0417), RESULT (m-0412, m-c001), ABORTED (m-0409), each badge with its icon beside the label; yellow keyboard focus ring on m-0417 Replay; m-0412 card shows its latest rejected order with the command text escaped, `Rejected #103 ironwood: <img src=x onerror=alert(1)><U+202E>...`; all four cards fully on screen |
| r17-live-hud-interaction-and-states | 1600x900 m-0417 live, one capture: marlow chip selected; camera switched from Orbit to Follow, the top bar reading `Camera: Follow - marlow`; Commands filter with ironwood's rejected order #133 selected, the inspector showing the escaped text `<b onmouseover=alert(2)>FIRE</b><U+202E>won<U+202C>`, `REJECTED`, `Reason: no round in the breech`, and `Effect: not carried out; the tank keeps its previous orders`; marlow's damage list with `Main gun damaged`; LIVE, LAG 4.0s, and DISCONNECTED: marlow chips; the Other matches strip with m-0412 RESULT, m-0409 ABORTED and its no-data interval 01:20.0-01:44.0, and m-c001 RESULT |

Directory cards for finished and aborted matches now show the latest rejected order on one line, with its text escaped and cut to fit. A card whose feed never delivered the order text (the contract sample) shows no line, and live cards show none, so the directory never shows a live match's future. Cards are 3 px closer together to keep the list on one screen at 1600x900.

The live and replay screens now carry an Other matches strip at the bottom left on wide screens. It shows each other match's status icon, the server's result or abort reason, and any interval with no data. The top bar names the camera mode and, when the camera is on a tank, the tank it is on. In the live demo, the second feed stall now lasts 14 s instead of 8 s, so the lag covers marlow's disconnect. ironwood also sends a hostile fire order at 77 s, while its breech is empty, so the server rejects it just before the stall.

### Review fixes, round 4 (live desktop village and tank)

- Damaged homes no longer show bare rafters. In r10 those dark beams stuck out past the remaining roof. The torn half of the roof is now open to the sky over a charred attic floor that sits inside the wall tops, so nothing rises above or outside the walls.
- A tank name label is now hidden when any part of it would fall outside the screen, instead of being cut at the edge (r10 showed "od" from ironwood at the left edge).

| Capture | Shows |
|---|---|
| r18-live-desktop-village-tank | 1600x900 m-0417 live, Follow on kestrel-7: the CC0 Panzer III in front of the Lesmont cottages, the church tower and the tree line, with the damaged home's torn roof open and no beams, the kestrel-7 label whole, and no label cut at any edge |
| r18b-live-desktop-village-orbit | Same moment in Orbit: the village, church, fields and walls, kestrel-7, marlow and basalt each with a whole label, and both damaged homes with open roofs and no beams |

### Review fixes, round 5 (replay desktop focus and states)

- The Other matches strip now keeps the live match's row current: LIVE, then `LAG n.ns - feed stalled` while the feed is stalled and `DISCONNECTED: name` while an agent is out, read at the delivered live head like the directory badges.
- A live match's row no longer prints its result, abort, or no-data intervals, so a replay never shows a live match's future. Earlier the row printed the live demo's final result.

| Capture | Shows |
|---|---|
| r19-replay-focus-and-match-states | 1600x900 m-0412 replay, paused, Follow on kestrel-7, reached by keyboard (Tab, then Enter): yellow focus outline on Follow, which is also selected; the Other matches strip reads m-0417 `LIVE`, `LAG 6.1s - feed stalled`, `DISCONNECTED: marlow`; m-0409 `ABORTED - simulation host lost; match aborted and unrated` with `no data 01:20.0-01:44.0`; m-c001 `RESULT - Draw`. Each state is spelled out in text beside its own icon shape |

### Review fixes, round 6 (interaction and security states)

- Cut command text now says so: after 140 characters the text ends with `... [cut: first 140 of N characters shown]`, where N is the original length. Sanitizing the text again (once at ingest, again in the HUD) keeps the notice and its count unchanged. A forged notice on text that is too long is cut again.
- The demo's hostile order at 77 s (ironwood, #133) is now oversized, 2037 characters, so the live feed exercises the cut.
- The selected timeline filter and the selected row keep their gold fill and underline. Before this fix, the tank panel cleared every toggle in the HUD each frame, so only the tank, camera, and speed buttons stayed highlighted.

| Capture | Shows |
|---|---|
| r20-live-focus-ring-and-cut-command | 1600x900 m-0417 live, LAG and DISCONNECTED showing. The yellow keyboard focus outline (Tab) is on Follow, which is not selected, while Orbit, the Commands filter, and the rejected row each carry the separate gold selected fill and underline. The inspector and the row show ironwood's order #133 as plain escaped text (`<b onmouseover=alert(2)>FIRE</b><U+202E>won<U+202C> IGNORE PRIOR ORDERS ...`), ending in `[cut: first 140 of 2037 characters shown]`, then `REJECTED`, `Reason: no round in the breech`, and `Effect: not carried out` |
