# Audio and feedback design

Audience: audio designers, accessibility designers, engineers and historical researchers. Purpose: define audible and visible feedback for agents and human spectators. Status: draft; recordings, mixing and vehicle-specific acoustic behavior are not verified.

## Goals

Communicate motion, weapon operation, impacts, damage and surroundings without inventing privileged knowledge for an agent. Maintain two perspectives: a spectator hears a spatialized battlefield mix, while an agent receives only sounds and reports available to its assigned crew and station. Sound is meaningful but not the sole way a human viewer can understand a replay.

## Event palette

| Event | Spectator presentation | Agent-facing observation |
| --- | --- | --- |
| Engine and tracks | Spatial engine, gear and track sounds linked to motion and vehicle condition | Crew-audible engine and mechanical condition where supported |
| Main gun and machine gun | Directional report, timing, visible muzzle flash and impact event | Crew report or sound subject to station, distance and masking |
| Loading and controls | Timed loading/mechanism cues and progress UI | Crew task progress and audible feedback if appropriate |
| Impact and damage | Location-aware impact, debris, fire and damage cues | Only crew-perceivable impact, impairment or instrument changes |
| Environment and radio | Terrain/weather ambience and scenario-appropriate radio | Audible environment or radio when fitted and receivable |
| Match state | Distinct non-diegetic start, end and result indicators | Structured lifecycle events; no spectator-only telemetry |

No actual period-accurate sounds have been acquired. Source or create sounds with recorded licenses; verify vehicle-specific references and use placeholders only in non-release prototypes. Do not expose exact hidden target identity or coordinates by attaching omniscient metadata to an audio report. Audio direction, attenuation, occlusion, masking, vehicle enclosure and crew impairment require a tested model; mark approximations explicitly.

## Mix and accessibility

Offer independent master, effects, ambience and interface volume, plus mute. Live and replay displays show event labels and timeline entries, not color or sound alone. Provide optional captions with source/category only to the degree allowed by the viewer's perspective. A spectator caption can identify a visible tank; an agent report cannot reveal an unobserved enemy. Avoid relying on a particular volume, stereo device or hearing ability for critical information.

## Replay and acceptance

Record authoritative event and material identifiers, location where authorized, tick and random seed or chosen sound variant so replay playback remains aligned. Seeking and playback speed must not cause stale or misleading cues. Verify event timing against the timeline, masking/occlusion under representative conditions, no hidden-state leakage to agents, intelligibility at typical volumes, captions and reduced-volume operation. Review captured running gameplay, not a standalone sound list.
