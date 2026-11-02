# Simulation and crew design

Audience: systems designers, historical researchers and engineers. Purpose: define the tank-team abstraction and combat rules. Status: draft; vehicle-specific values and mechanisms are unknown until supported by sources.

## Simulation contract

The server advances a proposed fixed 20 ticks per second irrespective of agent requests. The server is authoritative over movement, line of sight, task scheduling, firing, damage and results. An agent submits an order; acceptance means it was recorded, while actual completion depends on the evolving vehicle and environment. Inputs that hold a control have a finite lifetime and return to neutral on expiration or disconnect. Server failure pauses match time; if recovery fails the match is aborted and unrated.

## Crew abstraction

A single agent coordinates the historical crew for its exact vehicle. Crew stations are roles within the tank, not separate players or agent avatars. Exact crew count and role overlap are vehicle data, not global defaults.

| Crew function | Typical work to model | Constraints |
| --- | --- | --- |
| Driver | Start/stop engine; select gear; steer, accelerate and brake | Engine/transmission, tracks, terrain, visibility, injury and control availability |
| Gunner | Select sight, traverse turret, elevate and aim, operate available weapon | Traverse/elevation limits and speed; sight obstruction; injury; stabilized or unstabilized equipment only where sourced |
| Loader | Select accessible ammunition, load a round, operate applicable equipment | Available rounds, work in progress, injury, inaccessible or damaged storage and vehicle-specific crew duties |
| Commander | Observe, select stations, coordinate and report | Viewpoint, hatch state, exposure and shared duties |
| Radio operator / other | Operate fitted radio or other assigned equipment | Equipment availability, range and crew workload where sourced |

One crew member can perform only work compatible with their roles and concurrent workload. Independent crew may operate in parallel. Starting an incompatible task rejects or interrupts it according to a documented vehicle rule; a crew loss can prevent actions or trigger supported reassignment. Expose the reason and progress rather than silently producing impossible behavior.

## Vehicle and combat state

A vehicle specification pins variant, configuration and sources for geometry, mass, propulsion, transmission, steering, crew and stations, optical devices, weapon arcs and handling, ammunition and stowage, fuel, armor and vulnerable subsystems. Values can be measured, derived or approximated with provenance and uncertainty. Unknown fields block a claim that the variant is historically validated.

Simulation covers terrain contact and obstacle collision; propulsion, speed and turning; gun handling, ammunition and reload; projectile travel and interaction with armor and environment; fire, crew condition and subsystem damage. Damage may impair movement, aiming, vision, communication or crew tasks. Exact thresholds and interactions are content data and historical research, not universal invented numbers. Wrecks remain represented in the world and replay; whether they obstruct movement or projectiles is a per-scenario physics rule established by testing.

## Orders and action states

An order progresses through `accepted -> started -> completed`, or ends as `rejected`, `interrupted` or `expired`. Each has an ID, server tick, owner, reasons and effects. Examples: movement input expires unless renewed; a traverse order respects mechanical limits; a fire order fails if no round is chambered or the weapon cannot fire; reloading takes time and may be interrupted. The simulator, not the client, decides validity at execution time.

## Fuel, ammunition and depots

A tank starts with a scenario-defined loadout and source-backed capacity. Firing consumes the chambered round. Loading moves a valid onboard round to the breech, takes crew time, and cannot occur with unavailable crew or ammunition. Fuel decreases according to the validated engine/movement model and may be affected by damage; empty fuel limits engine-driven mobility. Conservation is checked across loading, firing and any transfer.

Initial ranked scenario: no depot. A future depot is a scenario abstraction with stock, range, access, duration, interruption and contest rules specified in advance. The tank must physically arrive and remain eligible while supplies transfer; crew never leaves the agent's control boundary. Record transfer progress and cancellation for spectators and replays.

## Acceptance

Independent source fixtures and explicit tolerances cover the exact configuration's movement, optics, weapon handling and damage; unknowns remain visible. Tests demonstrate concurrent independent crew work, incompatible tasks, time passing without requests, TTL expiry, fuel/ammunition conservation, blocked firing, damage consequences and replayed order outcomes. No document alone proves historical accuracy.
