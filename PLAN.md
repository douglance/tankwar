# Tankwar plan

Audience: product owner and implementers. Purpose: build a browser-viewable, real-time WWII tank combat ladder operated by autonomous agents. Status: research and architecture draft, not an implemented or historically validated simulator. This plan uses the technical and developer documentation profile.

## Product contract

- Each agent controls one whole tank crew through Incurs CLI or MCP. Commands represent crew orders and available controls; crew actions take time and may conflict. The agent never leaves the vehicle. Views are limited to historically available positions, optics, apertures and crew observations; no omniscient tactical map or enemy state.
- Visual direction: use the user-favored low-poly WWII tank style in [IsolatedSoulStudio’s pack](https://isolatedsoulstudio.itch.io/lowpoly-tank-pack-01) as a reference; historical vehicle capabilities, crew limits and combat behavior remain accuracy targets. Asset reuse requires a verified license.
- Matches run continuously without turns. Support one tank per side and two tanks per side; each tank has its own agent identity. Humans can replay every fight in a browser after it ends; the browser is not authoritative. The environment is viewed in those replays and through the tank crew’s live views.
- Target all WWII tank types and historically faithful behavior. This is an open-ended research target, not a verified claim: a definition of variants, theaters and service dates and sourced vehicle data are required before coverage or full accuracy can be measured. Do not invent missing data or present approximations as facts.
- Collect, license-check, and inventory all assets for the agreed release roster and arena before implementing that release. The entire wartime roster and complete free assets are not yet established; the asset-first gate currently blocks game implementation.

## Architecture

```text
+---------------------+     +-------------------------+
| Incurs CLI / MCP    | --> | Authenticated API       |
+---------------------+     | Cloudflare Worker       |
                            +------------+------------+
+---------------------+                  |
| Browser viewer      | -----------------+
+---------------------+                  v
                            +-------------------------+
                            | Match coordinator and   |
                            | authoritative simulation|
                            +------------+------------+
                                         |
                            +------------v------------+
                            | Results and ladder      |
                            +-------------------------+
```

Serve the web client and versioned assets through Workers and object storage. An Incurs command graph supplies CLI, MCP and HTTP interfaces; verify the chosen Incurs version and transport compatibility in a prototype. A Worker authenticates requests, authorizes every tank and match action, and routes to one authoritative match actor. A Durable Object per match is the initial experiment, not a guaranteed production physics host. A matchmaking/ladder actor serializes queue assignment and rating updates; durable database storage retains identities, match results and ratings. Object storage holds assets and replays. Persist every completed fight with the initial state, simulation/content version and ordered events needed to reproduce it, and provide an indexed human replay library. Test replay completeness and playback across versions; settle whether failed or interrupted fights also appear before launch. Replay cameras may show the whole battlefield after the match, but must never expose hidden state to an agent during a live match.

A match runs a fixed simulation step independent of command arrival. Agents submit ordered, time-limited intent; observation and rendering are projections of the same server state. Attach monotonic sequence numbers and unique command IDs; acknowledge accepted/rejected commands and make retries idempotent. Expire held controls to neutral on disconnect. Gate late, duplicate, impossible, unauthorized and out-of-match commands. An agent can reconnect to its assigned seat, but cannot take another seat. Define before release: clock synchronization, rollback/recovery policy, input deadlines, dropped-client and server-failure outcomes, simulation version and replay retention. Never trust client hit, position, visibility or victory claims.

Workers/Durable Objects lifecycle, CPU limits, timer jitter, memory and price require an actual long-running fixed-step load test. Measure 1v1 and 2v2 representative terrain/physics, CPU per simulated second, tick lateness p95/p99, command-to-effect latency by geography, bandwidth, failure recovery and cost per match. Set numeric budgets before choosing an engine. If it cannot meet them, retain Workers for edge/auth/matchmaking and run the same authoritative protocol on a persistent simulation host; do not claim a Workers-only solution until demonstrated. Durable Object alarms are not assumed to be a high-frequency game clock.

## Identity and authorization

Use a real identity provider and OAuth authorization server. Human owners authenticate and explicitly authorize a named agent/client; issue short-lived, audience-bound and scoped access tokens, with revocation and refresh. Remote MCP uses the OAuth flow and protected-resource discovery; CLI uses an OAuth authorization-code flow with PKCE, local callback or a supported device flow. Headless service credentials require an explicit supported enrollment mechanism; do not assume every provider supports device authorization or client credentials. Verify issuer, audience, expiry and scopes, then check account, agent, queue, match and seat ownership server-side. Separate scopes for profile, queue, observations and commands. Store secrets in an OS credential store or protected agent secret store; never in command arguments, logs or replays. Apply per-agent and per-account limits and prevent cross-tenant observation. A real end-to-end login, revocation and denied cross-seat test is a release gate.

## Agent interface

Illustrative command names, not implemented syntax:

| Group | Orders and observations |
| --- | --- |
| Identity | login, logout, identity |
| Competition | queue join/leave, match status, ladder, results |
| Observe | crew reports, available viewpoints, look through a selected sight/periscope/hatch, inspect instruments and controls |
| Operate | drive/throttle/steer/brake/gear, engine, turret traverse, gun elevation, sight setting, select/load ammunition, fire, crew orders, available radios and hatches |

Expose legal controls and preconditions per vehicle and configuration, including equipment failures and unavailable crew. Individual commands are bounded actions or expiring continuous intents, with timestamps, command ID, structured errors and progress. Model the vision devices fitted to each exact vehicle and configuration: periscopes, vision slits, gun sights and open-hatch views where historically supported, including their direction, field of view, magnification, occlusion and exposure. The [National Army Museum describes the Mk IV periscope](https://collection.nam.ac.uk/detail.php?acc=1975-03-63-2-587), and the [M4A3 technical manual](https://curioandrelic.com/fieldmanuals/tm9-vehicles/TM9-759-m4a3-sherman.pdf) is an example of vehicle-specific source material. Observation returns only what the crew could perceive, with field of view, occlusion, timing, optics, sound and impairment; avoid returning hidden world coordinates, enemy identity or internal state. Support image observations and structured crew reports derived from the same visibility rules. A crew abstraction may coordinate concurrent driver, gunner, loader and commander work, but must enforce crew capacity, vehicle-specific limits, reload and traverse times. MCP clients and CLI use the same authorization and domain contracts. Offer a low-latency streaming interface only after its auth and parity are proven; ordinary request/response remains usable.

## Ladder

Maintain separate 1v1 and 2v2 ratings per agent and season, with a visible leaderboard, match history, provisional status, and immutable result ledger. Initial proposal: 1500 rating, K=32; expected score `1 / (1 + 10^((opponent - own)/400))`, update `K * (actual - expected)`. For 2v2 use the arithmetic mean of teammate ratings for each team's expectation, then apply each player's K update against that team expectation. Treat draws as 0.5. Serialize result application exactly once by match ID and record pre/post ratings, rule version and correction events. Decide and publish placement, matchmaking range expansion, inactivity, surrender, disconnect, abuse controls, season reset and correction policies before ranked launch. Elo does not itself prevent collusion, multiple accounts or rating inflation; measure and mitigate those separately.

## Asset-first inventory and gates

See [assets/README.md](assets/README.md) for downloaded files and provenance. A material is not an arena. For every asset retain original URL, creator, exact license and version, download date, checksum, file format, source archive and all license/attribution obligations. Verify author rights and game redistribution, modification and commercial terms; free-to-download is not a license. Check each model for exterior, required interior, moving components, scale, collision suitability, damage variants and historical references. Check terrain for usable geometry, rights, period-appropriate buildings, vegetation and roads. Capture individual source licenses rather than inferring an entire marketplace has one license. Never use ripped game assets. Document gaps explicitly and commission or create assets where suitable free sources cannot be found; that changes the free-assets objective and requires a decision.

The discovery gate needs: a defined exhaustive tank/variant taxonomy; documented primary sources for each vehicle and theater; asset inventory with independent availability/licensing and historical review; terrain heightmap and complete arena; interior, sights, crew interfaces, audio, effects, animations and destructible geometry. At present, no complete arena, tank roster, tank interiors or source dataset has been verified. Therefore the user's complete asset collection is not satisfied.

## Work sequence and acceptance

1. Define the roster taxonomy, theaters, supported configurations and historical evidence standard. Inventory all required assets and download only individually verified, redistributable files. Gate: every required item is present, hashed and reviewed, or its gap is resolved.
2. Prototype Incurs and authenticated CLI/MCP plus a server-authoritative fixed-step match. Gate: independent agents can authenticate, continuously drive and observe without cross-seat leakage; load and recovery measurements meet pre-agreed budgets.
3. Build one historically sourced vertical slice, including browser viewer, crew controls, arena, physics, damage and replay. Gate: compare independently measured behavior against cited vehicle references with explicit tolerances; blind visibility and exploit tests pass. Establish visual acceptance using captured running evidence.
4. Add 1v1/2v2 matchmaking, durable Elo ledger, replay and abuse controls. Gate: concurrent queue, disconnect, replay, duplicate-result and rating-calculation tests pass, including fault injection.
5. Expand the full catalog through the same asset, data and accuracy gates. Track coverage as sourced variants / agreed catalog; report unknowns and discrepancies, never blanket accuracy.

For each suite, deliberately introduce and verify a representative mutation, then demonstrate a failing test before restoring the tree. Test deterministic simulation with independently sourced reference cases, network delay/loss, authentication and ownership, visibility, race conditions, conservation of match results and unauthorized requests. Browser visual claims require captures of the running build, not reference art.

## Source notes

- [Cloudflare MCP authorization](https://developers.cloudflare.com/agents/model-context-protocol/protocol/authorization/) and [MCP security](https://developers.cloudflare.com/agents/model-context-protocol/guides/securing-mcp-server/): OAuth and resource checks.
- [Durable Object limits](https://developers.cloudflare.com/durable-objects/platform/limits/) and [WebSockets](https://developers.cloudflare.com/durable-objects/best-practices/websockets/): lifecycle and capacity constraints to validate.
- [Poly Haven license](https://polyhaven.com/license): the identified terrain material is CC0; its site and API have separate terms.
- Incurs source inspected locally at `/Users/douglance/Developer/lv/incurs/README.md`; its exact release and integration remain to be pinned and verified.
