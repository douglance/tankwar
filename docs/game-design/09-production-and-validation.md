# Production plan and validation

Audience: producers, designers, engineering, art, research and quality. Purpose: sequence work and identify evidence needed for release. Status: proposed; duration, team size, budget and launch date are unknown.

## Production gates

| Phase | Deliverable | Exit evidence and dependency |
| --- | --- | --- |
| 1. Content discovery | Exact first-release vehicle/configuration list, one scenario, complete asset and source manifest | Every release asset acquired, local, rights reviewed and visually/historically assessed; no missing required item |
| 2. Technical feasibility | Non-game prototypes for authentication, Incurs/MCP, fixed-step host, observation renderer and persistence | Real login/revocation and seat isolation; measured 1v1/2v2 load, time, failure, replay and cost against [benchmark targets](../../BUILD_PLAN.md) |
| 3. Gameplay slice | One sourced vehicle and complete scenario, crew actions, perception, combat, viewer and replay | Independent reference cases, collision/visibility and crew tests; browser capture and visual review |
| 4. Competitive release | 1v1/2v2, queue, Elo, full spectator/debug/replay and operations | Concurrency, result exactly-once, auth, recovery, replay, accessibility and operational acceptance |
| 5. Catalog expansion | Additional sourced vehicles, configurations and theaters | Same content, accuracy, integration, replay and performance gates per release |

User requires assets first for each release; a non-game proof of hosting or auth is research and may proceed while assets are being collected. A vehicle exterior alone is not a vehicle-completeness gate. Full wartime roster and full accuracy remain product targets and need an exhaustive catalog, rights, source data and tolerances before they can be measured.

## Key risks and planned response

| Risk | Effect | Planned response |
| --- | --- | --- |
| Required free assets do not exist or rights are unclear | Release blocked | Search and record gaps; create missing assets internally or seek an explicit decision on paid sourcing; verify the provided Panzer IV license |
| Simulation host or server-side sight rendering misses budget | Match timing or perception fails | Benchmark representative 2v2 and rendering early; retain Workers as gateway and use a persistent host if needed |
| Historical data is incomplete or conflicting | Unsupported authenticity or unusable variant | Record source, exact configuration, uncertainty and independent verification; exclude or label approximations |
| Public live debug leaks hidden information | Competitive unfairness | Explain limitation, monitor abuse and separate agent API visibility from spectator feed; do not promise secrecy |
| Replay append or storage fails | Lost accepted commands or fights | Gate acknowledgments on durable append, pause on failure, reconcile immutable segments and test fault boundaries |
| Extensive content scope | Delivery delay or unsupported completeness claim | Ship sourced, complete increments while tracking full catalog gaps; never substitute early slice for full target |

## Quality strategy

Use source-derived acceptance cases for each historical parameter, with tolerances and independent expected results. Exercise agent observation from each supported station and confirm no hidden data leaks via images, reports, errors or timing. Test crew scheduling, fuel/ammo conservation, command TTL and idempotency, damage, 1v1/2v2 outcomes, equal and unequal Elo calculations, duplicate match results, reconnect, storage interruption, replay seeking and version upgrades. Test each meaningful suite by confirming a representative mutation applies and makes a relevant check fail, then restore the implementation.

For documents or visual artifacts, use validator with a reviewed and confirmed rubric and actual evidence; unknown and error are not pass. For visuals use captures of the running build. For authentication and online behavior test a real environment, not just a local emulator. Record exact revisions, environment, results and known limits.

## Unresolved decisions

- First arena's exact theater, date, season, dimensions and available period references.
- Exact first-release variants, sight arrangements, source data and legal rights for the supplied Panzer IV.
- Auth provider and supported headless enrollment flow; Incurs exact release and transport compatibility.
- Simulation engine and production host after measured CPU, latency, renderer and cost tests.
- Realism tolerances, complete catalog definition, accessibility standards and operating budget.

Proposed defaults in the game documents are design hypotheses, not evidence that these decisions have been validated.
