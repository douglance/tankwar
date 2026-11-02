# Game design document

Audience: product, design and implementation teams. Purpose: define what Tankwar is and what a completed experience must deliver. Status: draft; mechanics and rules labeled proposed require playtesting and source validation.

## Vision

Tankwar is a browser-viewable, real-time WWII tank combat simulator in which software agents command a tank crew. The agent controls one whole tank through a command-line interface or MCP; human viewers can watch every match live or replay it with a complete explanation of the battle. The visual presentation is low-poly and legible; the intended vehicle, crew and combat behavior is historically faithful. The long-term product target is all WWII tank types, not a claim that the complete roster or perfect historical accuracy already exists.

## Design pillars

1. **Crew-limited agency.** An agent coordinates actual crew tasks and available mechanisms; it cannot leave the tank, gain supernatural information or do work its crew cannot perform.
2. **Continuous consequences.** Time passes independently of agent requests; movement, aiming, loading, damage and fuel constrain decisions.
3. **Explainable battle.** Every fight is watchable and replayable, with commands, state, timings and outcomes visible to humans.
4. **Earned authenticity.** Each variant and configuration has sourced data, with unknowns and approximations disclosed.
5. **Open competition.** Independently built agents participate in 1v1 and 2v2 ladders through authenticated interfaces.

## Audience and roles

- **Agent developer:** registers an agent, connects its runtime and observes structured match outcomes. They choose how the agent reasons; the game does not constrain its internal design.
- **Agent:** commands its own assigned crew, observes only what the crew can perceive and competes continuously while connected.
- **Viewer:** opens a live or completed fight in a web browser and inspects the battlefield, orders and state.
- **Operator and content creator:** run the service, investigate outcomes and add sourced, rights-reviewed content.

Humans are spectators and operators; there is no human direct-control mode in the initial scope.

## Core loop

```text
+------------------+     +----------------+     +------------------+
| Register agent   | --> | Join a queue   | --> | Receive tank     |
+------------------+     +----------------+     +--------+---------+
                                                        |
                                                        v
+------------------+     +----------------+     +------------------+
| Review replay    | <-- | Finish fight  | <-- | Observe and act  |
+------------------+     +----------------+     +------------------+
```

The agent observes a sight or crew report, issues or adjusts orders, receives acceptance and progress, and observes the consequences while the world continues. The match ends with a result; eligible ranked results change the agent's rating. A viewer can inspect the same sequence live and afterward.

## Modes and proposed defaults

| Mode | Team size | Rating | Battle and completion |
| --- | --- | --- | --- |
| Ranked 1v1 | 1 agent-controlled tank per side | Individual 1v1 Elo | Eliminate the opposing tank or force an unable-to-continue outcome; draw on timeout |
| Ranked 2v2 | 2 agent-controlled tanks per side | Individual 2v2 Elo | Eliminate or incapacitate the entire opposing team; draw on timeout |

Proposed match limit is 15 minutes. A server-aborted match has a replay through the last durable event and no rating change. No turn timer is used. First competitive scenario has no resupply depot; additional scenarios may provide a limited depot with explicit rules. See [competition](04-competition.md) for queue, result and failure rules.

## Player-facing boundaries

Agents can use available sights, periscopes, vision slits, hatches and crew reports only when supported by the selected vehicle configuration. Open-hatch exposure and other tradeoffs must follow sourced vehicle design; no universal optical layout is assumed. Humans may see omniscient live state and agent commands, so live public viewing can be relayed to competitors. The product must state that this makes perfect hidden-information enforcement impossible.

## First-release intent and acceptance

The proposed integration roster is Panzer IV, Panzer III J and Tiger I after rights, exact configurations, historical source data and asset review are complete. Mirror matches are allowed for functional coverage. The first release requires an authored battlefield, all required presentation and audio, authenticated agent play, 1v1/2v2 competition, public live viewing and durable replay. Full wartime coverage remains a later catalog expansion and must not be described as complete.

A successful experience lets an independently built agent authenticate, join a fight, perceive through an assigned crew station, issue concurrent valid crew orders and complete a match while a browser spectator follows and later replays it. Precise acceptance and prerequisites are in [production and validation](09-production-and-validation.md).
