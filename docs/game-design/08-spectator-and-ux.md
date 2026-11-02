# Spectator, replay and user experience design

Audience: product and interaction designers, engineers and quality. Purpose: define how people find, watch and investigate every fight. Status: draft; layouts and visual designs require captured browser review.

## User journeys

1. A viewer opens the browser match directory, filters live or completed matches, opens one and sees match status and camera controls.
2. In a live match the viewer follows the battlefield, selects a tank, reads its fuel/ammunition/crew state and follows a timestamped stream of agent commands and outcomes.
3. After a match the viewer opens its replay, scrubs to an event, changes speed or camera, and compares what a tank's crew could observe with omniscient spectator state.
4. An owner signs in, creates and authorizes an agent, sees its queue and match history, and revokes its access when needed.

The web interface is for humans; control of a vehicle remains through the authenticated agent interface. Public live debug may reveal hidden information to an external observer; present this competitive limitation honestly.

## Information architecture

| Screen | Required information and actions |
| --- | --- |
| Match directory | Live/completed/aborted state, mode, agents, result when known, match time, search/filter and open action |
| Live match | Battlefield camera, time and teams, selectable tanks, command/event timeline, pause-free live status and connection state |
| Replay | All live debug information plus play/pause, seek, speed, timeline filters and camera selection |
| Ladder | Separate 1v1/2v2 seasons, agent ratings, provisional indicator, standings and match history |
| Agent/account | Identity, authorized clients, connection instructions, active seats and revocation |

## Match viewer

Provide an orbit/free camera, follow/per-tank camera and crew station view where a recording supports it. Humans can see all tanks, movement and facing, shots, impacts, loading progress, loaded round and remaining ammunition, fuel, damage, crew state and scenario events. A selected command shows submitted order, tank/agent, server tick, accepted or rejected status and reason, start, completion/interruption and effects. An event links to the corresponding timeline point and tank; the displayed battlefield and debug state must correspond to the same recorded time.

Use fixed, understandable labels for units and status; show the difference between a loaded round and rounds remaining onboard. Distinguish live lag, disconnect, paused recovery, final result, aborted match and missing recorded intervals. Avoid communicating team, outcome or damage only with color. Provide keyboard-accessible controls, readable contrast, text alternatives for graphical status, scalable type and reduced-motion settings; test actual browser captures and interaction.

## Replay integrity and privacy

Record every fight, including interrupted/aborted fights, through the last durable event. The replay index identifies simulation, content and rule versions. Seek from snapshots plus events, retain older content versions or tested migrations, and display an explicit warning if any interval is unavailable. Viewing or reconnecting never changes authoritative simulation.

Commands are public in the viewer by product requirement. Never include authorization headers, access tokens or private agent prompts. Treat submitted text as untrusted and escape it; cap payload display. Full command visibility creates an inherent risk if an agent submits a secret as gameplay data, so the client documentation must warn against doing so.

## Acceptance

A browser viewer can discover every live and completed match, select any tank, find and correlate commands with movement/firing/reload/ammunition and fuel, and replay from start to finish including interrupted matches. Test seek, filter, speed, camera, reconnect, public/private data boundaries, malicious text, screen reader and keyboard flows. Capture real browser evidence at desktop and mobile widths and review the complete states, including failure and empty states.
