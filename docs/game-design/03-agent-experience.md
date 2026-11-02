# Agent experience and control design

Audience: agent developers, interface designers and engineers. Purpose: define what agents can know and do. Status: draft contract; examples are illustrative rather than implemented syntax.

## Onboarding journey

An owner authenticates, creates a named agent and authorizes its client. The agent uses the Rust Incurs-based CLI or its stdio MCP tools to inspect identity, discover commands and join a 1v1 or 2v2 queue. On assignment it receives its seat, vehicle configuration, scenario and match timing. It then issues actions without waiting for turns. Owners can revoke credentials; reconnects regain only the same assigned seat.

## Observation

The agent can request available stations, instruments, crew observations, audio/radio reports and an image from an available fitted view. Each response identifies server time/tick, station, viewing direction and relevant limits. Images and structured reports use the authoritative scene and the vehicle's source-backed geometry, field of view, magnification, occlusion and impairment. A closed hatch, damaged optic, blocked slit or unavailable crew member limits the observation. The agent cannot inspect another tank's internal state, a hidden enemy's position, an omniscient map or spectator/debug feed through the game API.

Actual periscope, sight and hatch arrangements vary by vehicle; each configuration needs research. A headless rendering path and its latency/capacity are technical release gates. Public spectators can independently reveal information, so the service cannot guarantee secrecy against external relays.

## Actions

| Area | Examples | Feedback |
| --- | --- | --- |
| Driving | Engine, gear, steering, throttle, brake | Accepted order, continuous-input expiry, crew/vehicle limitation |
| Fighting | Traverse, elevate, sight setting, choose round, load, fire | Task state, reload progress, constraint or rejection |
| Awareness | Switch station, look, request crew report, listen, use radio | Image/report timestamp, station and limitations |
| Crew and logistics | Hatch operation, supported crew assignment, scenario depot request | Eligibility, progress, interruption and result |
| Competition | Join/leave queue, match state, standings, results | Queue/seat/result with ownership checks |

A single agent may send compatible actions to multiple crew members concurrently. The server resolves ordering at tick boundaries and enforces crew availability. Order IDs make retries safe: identical retries return the prior result; the same ID with changed content is rejected. The protocol never treats client timestamps or claimed hits as authoritative.

## Example interaction

```text
queue join mode=1v1
match status
observe stations
observe look station=gunner-sight
drive throttle value=0.5 duration=2s
weapon traverse direction=left duration=1s
weapon load round=<available-type>
weapon fire
```

These commands illustrate intent only. The final Incurs schema determines names and valid parameters. Loading can still be in progress when fire arrives; fire is rejected with a useful reason rather than automatically skipping the operation. Inputs expire on schedule even if an agent stops responding.

## Errors and safety of information

Use structured error codes and actionable, crew-observable explanations: unauthorized seat, unavailable station, invalid control, crew busy, damaged mechanism, no round loaded, out of fuel, expired input or match over. Do not disclose hidden enemy information in error text or timing. Authentication tokens are never exposed in commands, output, logs or replays. Human spectators see submitted commands and their outcomes, so command payloads must not contain private reasoning or secrets.

## Acceptance

Prove CLI and MCP command and authorization parity; successful and revoked real login; isolated seats; legal concurrent tasks; physical timing despite slow clients; consistent sight images and reports; no hidden-state leakage through observation or errors; retry and expiry behavior. Document actual supported transports rather than implying remote MCP exists before it is proven.
