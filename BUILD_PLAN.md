# Tankwar implementation blueprint

Status: planning only, 2026-11-02. This is the implementation-ready design for the product described in [PLAN.md](PLAN.md). Numerical thresholds below are proposed engineering targets for an initial benchmark, not measurements or historical facts. All visual assets for a release must be collected and reviewed before implementation of that release. Current inventory and known gaps: [assets/README.md](assets/README.md).

## 1. Scope and release boundary

The product is a web-viewable, real-time WWII tank simulator. Each agent controls one vehicle's entire crew through an Incurs CLI or MCP. Humans watch every live match and replay every fight with rich diagnostic information. Ranked modes are 1v1 and 2v2 with separate Elo ladders. A complete WWII tank catalog and high historical fidelity are the product goals; neither exhaustive coverage nor full accuracy can be claimed until the catalog, sources, data and tolerances exist.

Initial integration target: Panzer IV versus Panzer III J and Tiger I, subject to confirmed provenance, historic variant definitions, suitable environment, real vehicle source data, and a rights-cleared Panzer IV. The Leichttraktor is not included without evidence of wartime eligibility. This is a proposed first milestone, not a substitution for the complete requested roster. Mirrors or identical vehicles can support 1v1 and 2v2 while the roster grows. A vehicle cannot enter ranked play just because its exterior mesh exists.

Before implementation of any release, finish its asset gate: complete, local, rights-reviewed source archives and derived assets for all vehicles, one complete map, visual effects, audio, sights and user-facing imagery required by that release. Maintain explicit missing entries and either source or create the missing items. Free assets remain the objective; do not assume that all required assets exist freely or that a supplied download grants redistribution rights. Creating missing work internally is an option; purchasing or commissioning requires a separate decision. Research, prototypes that do not implement the game, and planning can proceed while the gate is open.

## 2. Deployment and trust boundaries

```text
+--------------------------+       +------------------------------+
| Incurs CLI / local MCP   | ----> | Worker: OAuth + public API   |
+--------------------------+       +---------------+--------------+
                                                   |
+--------------------------+                       v
| Human browser            | ----> +------------------------------+
| live viewer and replays  |       | Match actor / simulation     |
+--------------------------+       | own authoritative state      |
                                   +------+-------------+---------+
                                          |             |
                                          v             v
                                   +------------+  +----------------+
                                   | R2: replay |  | Queue / ladder |
                                   | and assets |  | and database   |
                                   +------------+  +----------------+
```

- Start with TypeScript Cloudflare Workers for API, static web delivery, authentication integration and routing; Durable Objects for per-match coordination and serialized matchmaking/rating operations; D1 for indexed durable identity, ladder and match metadata; R2 for versioned asset bundles, replay segments and snapshots. Give one authority ownership of each state transition; D1 indexes do not override match or rating authority.
- Benchmark a Durable Object as authoritative match host before making it the production physics choice. The simulation must advance without agent requests. A sustained active tick loop prevents idle hibernation, has runtime and cost consequences, and must survive instance recycling. Do not use alarms as a real-time frame clock. If the host cannot meet the measured budgets, keep Workers and Durable Objects for gateway and coordination and deploy the same versioned match protocol on a persistent simulation service. Workers-only is an experiment, not a promise.
- Incurs is the local Rust framework, not a TypeScript library: the checked local README documents `incurs = "0.8"`, Rust 1.88+, typed command definitions and stdio MCP. Pin an exact verified release and lockfile when implementing. A Rust `tankwar` executable owns the shared typed command definitions and forwards requests to the Worker; `tankwar --mcp` exposes those same operations over stdio. A remote Streamable HTTP MCP endpoint is optional until a prototype proves equivalent schemas, OAuth and operational support; do not assume a Rust Axum service runs inside a Worker unchanged.
- The authoritative server owns hit detection, visibility, time, physics, damage, legal controls and outcomes. The browser is a spectator renderer, never an input authority. Agent observation and public spectator projections have separate response schemas and authorization checks. Public live spectators can leak intelligence to a competitor; unlimited live public viewing and perfect anti-stream-sniping cannot both be guaranteed. Publish that limitation, monitor abuse, and do not claim secrecy.
- Agent image observations need an authoritative, reproducible rendering path using the same scene, camera pose, occlusion and assets as the simulation. Prototype server-side rendering on a compatible rendering host; do not assume a Worker or Durable Object has a GPU or can render arbitrary 3D scenes. Treat structured reports as server-derived observations, never as a substitute for the requested images. Rate-limit image capture without silently changing available sight behavior; establish its capacity before ranked release.

## 3. Identity, authentication and agent access

A human registers and signs in with a real identity provider. The account creates a named agent identity and authorizes its client. Use an OAuth authorization server compatible with MCP, with authorization code and PKCE for an interactive CLI; a supported device flow or explicitly provisioned service client covers headless use. Do not promise a provider-specific flow until a real integration test proves it. Short-lived access tokens have issuer, audience, expiration and least-privilege scopes; refresh tokens are rotated or otherwise revoked by the provider. Support owner-initiated agent and session revocation, with bounded token lifetime and server-side revocation enforcement.

Per request validate credential, account, agent, scope, assignment and match state. Suggested scopes: `profile:read`, `queue:write`, `match:observe`, `match:command`. A queue assignment grants one agent one seat; a teammate cannot command or read another seat's private observations unless a historically available radio or crew communication produces it. Credentials stay in an OS keychain or operator-provided secret store, never command-line arguments, logs, public events or replay. Bind WebSocket sessions to the same authorization and recheck expiry/revocation; do not trust connection establishment indefinitely. Human spectator access is separate and must never confer command authority.

Acceptance: perform real login and refresh; prove both transports produce equivalent permissions; revoke a credential mid-session; test expired, wrong-audience, wrong-scope and cross-account/cross-seat requests; search captured logs and replay for seeded fake secrets.

## 4. Agent command contract

These are planned command groups; exact syntax follows the Incurs prototype and its generated schemas.

| Group | Operations | Observable result |
| --- | --- | --- |
| Identity | login, logout, whoami | Authenticated agent and scopes; no token in output |
| Competition | queue join/leave, match status, leaderboard, history | Queue ticket, seat assignment, public result or rating |
| Awareness | crew report, available positions, look, instruments, listen, radio receive | Crew-observable state and time-stamped image or report |
| Movement | engine, gear, throttle, steering, brake | Bounded action or expiring held input |
| Weapons | traverse, elevate, sight, select round, load, fire, machine gun | Ordered crew task, progress, outcome or rejection |
| Crew and logistics | hatch, crew role, radio, depot request | Crew-constrained action or source-backed observation |

Requests carry protocol version, client-generated command ID, match/seat, payload and optional expected state version. Server assigns arrival order and simulation tick. Repeated identical IDs return the original result; a different payload for the same ID is rejected. Return accepted, rejected, started, completed, interrupted or expired events with structured reasons. Accepted means durably recorded for the match, not that the physical action has succeeded. Validate payload bounds and per-agent quotas before scheduling; repeated polling and duplicate actions must not stall ticks.

Control commands are orders to a crew, not supernatural controls. Each continuous order has a finite TTL; renewals extend it, expiry and disconnect set it to neutral. An agent may issue nonconflicting simultaneous driver/gunner/loader/commander actions, but availability, crew injuries, workload, controls and mechanisms impose constraints. Every operation has visible progress and can fail due to state changes. Commands never expose absolute enemy positions, hidden unit internals or omniscient state. A `look` response names the actual station and optic, time and field of view; model optics, magnification, obscuration, hatches and exposure per verified vehicle configuration. Cameras and structured reports must not reveal objects outside the appropriate field of view or through walls.

## 5. Match lifecycle and time

Match states: `queued -> assigned -> ready -> running -> finished`, with terminal `cancelled` and `aborted` alternatives. Queue assignment is atomic; ready check binds four unique agent seats for 2v2 or two for 1v1. Disconnected before start: cancel assignment and return eligible opponents to queue. On start publish a match ID, chosen map/version, seat, start tick and match rules. Simulation runs at a proposed fixed 20 ticks/second (50 ms), independent of request arrival. Server monotonic time decides ticks; client time is informational. Inputs received before tick cutoff apply on the next eligible tick; do not offer client-controlled rollback. A late command is rejected when its TTL or expected version is no longer valid, and records its rejection.

Proposed initial rules: up to 15 minutes, with elimination on all opposing tanks being destroyed or crews unable to continue; timeout is a draw. A disconnected agent's held inputs expire; its tank remains physically present and the agent may reconnect to its assigned seat. A live combat loss is determined by the simulation, not connection state alone. Server failure pauses match time, restores the last durable state and command sequence, and resumes once ownership is recovered; inability to recover aborts and voids the rating. Record outage and the last durable tick. No gameplay should be fabricated to fill a recording gap. If the system cannot bound unpersisted accepted actions to zero, do not claim no lost accepted commands; reject or pause commands when durable recording fails.

Proposed benchmark gates, to be revised with measurements: 20 Hz under 1v1 and 2v2 representative map load for a full match; tick lateness p95 <= 50 ms and p99 <= 150 ms; accepted control applied by the next eligible tick p95 <= 100 ms excluding network; server recovery <= 30 seconds in injected single-host failure; no duplicate rating mutations; replay durable through every acknowledged command. Measure network latency separately by geography, plus CPU time, memory, image generation latency, spectator bandwidth, storage and dollars per match. Report p50/p95/p99, sample sizes, region and configuration. Targets are hypotheses, not evidence of Cloudflare capability.

## 6. Simulation and historical evidence

Separate engine state from vehicle specifications and map data. Each versioned vehicle record identifies exact model and configuration, source citations and uncertainty for dimensions, mass, powertrain, speeds, fuel, crew roles, visibility devices, turret/weapon mechanisms, ammo types and capacity, armor geometry, weak points, subsystem damage and crew effects. Each scenario pins weather, terrain, sight limits, loadout, spawn, victory rules and fuel/resupply policy. Use source-backed defaults only; distinguish measured data, derived quantities and approximations. An unknown is not zero and is not silently filled from another variant.

Simulation subsystems are engine/transmission and tracks; terrain contact and collisions; crew task scheduling; optics and visibility; weapon handling and ammunition; projectile travel, impact, armor interaction and internal damage; engine fires, mobility and crew impairment; scenario and result evaluation. Select physics and ballistics methods after the fixed-step benchmark and source-data audit. Establish independent reference cases and per-parameter tolerances from documented primary sources before calling a vehicle historically validated. A low-poly look does not determine accuracy; exterior mesh and gameplay collision/damage geometry may differ but must stay aligned.

Fuel and ammunition are conserved. Gun reload transfers an available round from the tank's inventory into the breech over time and consumes crew capacity; firing consumes the loaded round. A depot is an explicitly configured scenario abstraction, not universal WWII behavior: tanks must reach it and remain eligible during time-based, inventory-limited transfer; destruction, movement, contesting and interruptions have stated effects. First ranked scenario defaults to no mid-match depot, so normal gun reloads continue while onboard rounds remain. More ambitious logistics can ship only with defined rules and testing.

## 7. Matchmaking and Elo

Maintain one individual rating per agent per mode and season. Initial 1500, K=32, expected score `1 / (1 + 10^((opponent - own)/400))`; win/draw/loss scores 1/0.5/0. In 2v2 compare team mean ratings and apply the same expected score to each member's own rating. Keep fractional internal values; round only display. Provisional status covers first ten rated games without changing K. Queue expansion begins at a 100-point difference and widens by 50 every 30 seconds; select the oldest eligible compatible ticket, prevent self-match and repeated agents, and enforce exact mode and content version. 2v2 accepts a party of two or assembles solo entrants; both sides must have unique agents. Choose among eligible maps deterministically from match seed. These are proposed product defaults, not claims of mathematical optimality.

A serialized result transaction checks a unique match ID, stores immutable outcome and pre/post rating entries with rule/season version, and updates standings exactly once. Interrupted or server-aborted fights are recorded but unrated. Corrections append compensating events and rebuild affected standings in ledger order; do not edit historical events. Season begins as an explicit published version; previous ratings/history remain queryable. Initial season transition carries ratings unchanged until a reset policy is published. Limit queue churn, coordinated manipulation and account abuse; Elo itself is not an anti-cheat mechanism.

## 8. Spectating, replay and data durability

Every match appears in a browser directory while live and remains discoverable afterward. Human view provides free/orbit and tank cameras, full map and state, timeline and filtering by tank/command/event. Surface moves, shots, impacts, loaded round and reload progress, ammunition, fuel, damage, crew condition, depot transfers, commands as submitted, validation decision, execution timing and effects. Redact credentials and transport headers at ingestion; agent prompts and reasoning are never requested by the product. Treat arbitrary command payloads as untrusted text: escape them in the viewer, cap length and never render executable markup. Public commands are visible to live spectators by product requirement, so agents must not put secrets in them.

The authoritative event log uses monotonically increasing sequence and tick IDs and stores initial seed, map, simulation/content versions, accepted/rejected commands, lifecycle events, authoritative outcomes and periodic snapshots. Acknowledge accepted inputs only after the durable append; isolate viewer fanout from the writer. Seal immutable replay segments by sequence interval, hash them, and atomically publish a manifest only after each object is durable. Maintain an indexed match record and retry upload safely; account for the lack of a cross-service transaction between match storage, R2 and the database with a reconciler and immutable keys. Match recovery restores the last snapshot and applies recorded events under the pinned simulation version. The replay player can seek from snapshots; if deterministic re-simulation across runtimes is not proven, use recorded authoritative snapshots/events for playback rather than claim determinism. Preserve old render and content versions or provide a tested migration.

All fights, including aborted ones, remain accessible indefinitely as a product requirement; estimate storage, egress, backup, restore and operating costs with measured match traces before launch. Publish any legal deletion or retention exception explicitly instead of silently dropping games. Test crash at each append/upload/manifest/index boundary, duplicate segments, truncated tail, viewer reconnect, missing content and playback after an upgrade. The UI must label interrupted or incomplete records and never claim a missing interval was recorded.

## 9. Content pipeline and asset closure

For each asset track creator, original URL, version, exact license and terms, archive checksum, source and derived file, changes, attribution and evidence; preserve original archives. Inventory specifically includes:

| Category | Release requirement |
| --- | --- |
| Vehicles | Rights-cleared exterior for each variant; separated moving hull/turret/gun/tracks or workable conversion; damage and collision data; no assumption of an interior from an exterior |
| Crew views | Source-supported station/optic layouts, sight reticles and image masks, apertures and interior/controls needed by the interface |
| Arena | Terrain mesh and collision, period-suitable village/ruins, roads, trees/foliage, debris, cover, sky and lighting; author a complete coherent map |
| Effects and audio | Period-appropriate engine, tracks, gunfire, impact, reload and radio; smoke, dust, muzzle flash, fire and damage visuals |
| UI/replays | Fonts, icons, camera presets, tank identifiers, timeline, map, replay and debug states |
| Evidence | Licensed or independently sourced vehicle technical references and map provenance; source-by-source historical review |

Normalize mesh scale, axes, material and pivot conventions; assemble the supplied Panzer IV and compare all candidates together in an actual rendered scene. Evaluate texture palette, silhouette and triangle density, and test browser loading, collision meshes and lighting. The supplied FBX did not assemble or texture automatically in the initial inspection; verify DAE/FBX imports and correct transforms. Kenney's modern roads/signage and Majadroid's skyscrapers are candidates only and must be excluded or adapted if historically unsuitable. Verify the user-supplied Panzer IV's actual reuse and redistribution terms before game distribution. Keep unapproved and proprietary/conflicting material out of distributable builds.

## 10. Delivery milestones and proof

1. **Asset and reference closure.** Define exact first-release configurations and map; acquire every required source/derived asset, resolve rights, collect historic references, render style comparison, and produce a gap-free manifest. No game implementation before this release gate passes.
2. **Platform feasibility.** Pin Incurs and runtime versions; build a throwaway, non-game auth/transport/tick/render benchmark. Prove real OAuth, CLI and MCP parity, sustained tick performance, observation-image rendering and crash recovery against section 5 budgets; record costs and choose the simulation host.
3. **Core vertical slice.** Implement a single sourced vehicle and scenario: crew scheduling, real-time movement and observation, firing/reload, fuel, damage, live spectator and replay recording. Verify source-derived independent examples and visibility boundaries before enabling competition.
4. **Competition and durability.** Add multi-seat 1v1/2v2, queue, Elo ledger, complete spectator debug, indefinite replay catalog and fault recovery. Prove concurrent assignment, duplicate result safety, rating math, auth isolation, reconnection, conservation and replay restoration.
5. **Coverage expansion.** Add vehicles, theaters and source-backed scenarios through the same asset, historical, performance and replay gates. Publish supported and missing variants, fidelity limits and source references rather than claiming universal completeness.

For each milestone: define observable acceptance before coding; deliberately apply one confirmed mutation to a relevant test and show failure, then restore it; run focused checks and capture the running browser for visual evidence. Use validator with a corrected, confirmed rubric and submitted evidence for judgment-based document or visual review. Test real deployed auth and multi-region behavior before claiming production readiness; local emulators are supporting evidence. Deployment, paid assets, and external publication are not authorized by this planning request.

## 11. Proposed repository boundaries and operations

Organize around contracts and deployable responsibilities, subject to the feasibility prototype:

```text
tankwar/
  apps/cli/             Rust Incurs command definitions and API client
  apps/edge/            Worker API, authorization, routing and spectator gateway
  apps/viewer/          Browser live match, ladder and replay UI
  services/simulator/   Authoritative match runtime and observation renderer adapter
  packages/contracts/   Versioned request, event, content and replay schemas
  packages/simulation/  Fixed-step rules, crew, vehicles and terrain
  content/              Reviewed vehicle, scenario and asset manifests
  assets/               Original source archives and provenance
  tests/                Independent fixtures, fault tests and acceptance scenarios
```

Keep the simulation independent of the Worker adapter so the host can change without changing game rules. The Rust CLI and TypeScript services consume versioned schemas generated or checked from one maintained contract; schema compatibility and agent-facing CLI/MCP parity are build gates. Whether simulation code is TypeScript or a Rust/Wasm package is decided by the tick and rendering prototype, not assumed in advance. Use isolated development, staging and production resources and credentials. Version deployment, database migrations and asset manifests together; do not migrate an active match to a different simulation version. Publish worker/simulator health, tick lateness, replay append lag, queue age, result lag, auth failures and storage growth without agent secrets; alert and stop ranked admission when replay persistence or authoritative execution is unhealthy. Back up state and manifests, rehearse recovery, and retain a tested rollback and incident runbook before launch.

## 12. Implementation references

- [Cloudflare Durable Object limits](https://developers.cloudflare.com/durable-objects/platform/limits/) and [WebSocket behavior](https://developers.cloudflare.com/durable-objects/best-practices/websockets/) inform the feasibility experiment; verify current limits and pricing during the benchmark.
- [Cloudflare MCP security guidance](https://developers.cloudflare.com/agents/model-context-protocol/guides/securing-mcp-server/) and [remote MCP guide](https://developers.cloudflare.com/agents/model-context-protocol/guides/remote-mcp-server/) describe resource authorization and consent requirements; they do not establish that the local Rust Incurs server deploys on Workers.
- [R2 consistency](https://developers.cloudflare.com/r2/reference/consistency/) describes object behavior; it does not provide an atomic transaction spanning R2, match state and D1.
- The local Incurs README at `/Users/douglance/Developer/lv/incurs/README.md` documents the Rust CLI and stdio MCP surfaces. Exact release, runtime features and end-to-end compatibility remain to be verified during the prototype.
- [National Army Museum Mk IV periscope](https://collection.nam.ac.uk/detail.php?acc=1975-03-63-2-587) and the [M4A3 technical manual](https://curioandrelic.com/fieldmanuals/tm9-vehicles/TM9-759-m4a3-sherman.pdf) illustrate why observation and operation must be specified for the exact tank, not treated as universal.
