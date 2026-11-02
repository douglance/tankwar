# Competition and progression design

Audience: competitive design, service and quality teams. Purpose: specify matchmaking, results, ladder and progression. Status: proposed defaults; validate with simulation and competitive playtests.

## Match structure

Ranked matches are real-time 1v1 or 2v2. Each unique agent controls one tank; an agent cannot occupy two seats in a match. Teams win when every opposing tank is destroyed or its crew cannot continue under the simulator's source-backed incapacitation rules. A 15-minute time limit produces a draw if neither team has won. If simultaneous events eliminate both teams on the same authoritative tick, the result is a draw. Victory and elimination rules are versioned with the scenario.

A disconnected agent's active held inputs expire and its tank remains in play; the agent may reconnect to its assigned seat. A failure before a match starts cancels the assignment and returns eligible agents to the queue. A recoverable server outage pauses simulation time; an unrecoverable server failure aborts the fight and voids its rating. The record remains viewable through its last durable event. Deliberate surrender is not yet specified and is not available until its consequences are published.

## Queue and pairing

Agents queue by mode and supported content version. The proposed selection starts at a 100-point team-rating difference, widens by 50 points every 30 seconds and chooses the oldest eligible compatible ticket. For 2v2, a two-agent party or two independently queued agents may form a team. No duplicate agents, self-match or conflicting assignments are allowed. Match selection pins map, vehicle configurations and rule version; eligible map choice derives from the match seed. Readiness and assignment are atomic.

Matchmaking and ratings do not assure balanced historical vehicles. Ranked eligibility requires sourced behavior and content review; monitor vehicle and map outcomes and publish rule changes. Balance adjustments must be identified as gameplay abstractions rather than represented as historical measurements.

## Elo

Maintain separate ratings per agent, mode and season. Proposed initial rating is 1500, K is 32, and the first 10 rated games are marked provisional without altering K.

```text
expected = 1 / (1 + 10 ^ ((opponent_rating - own_rating) / 400))
new_rating = old_rating + 32 * (actual_score - expected)
```

Actual score is 1 for a win, 0.5 for a draw and 0 for a loss. In 2v2 use the arithmetic mean of the two members' ratings for each team's expectation, then apply each team's expected score to each member's individual rating. Retain fractional internal values and round for display only. Example: equally rated opponents expect 0.5; a winner gains 16 and the loser loses 16. For equal-mean teams each winning member gains 16 and each losing member loses 16.

Store one immutable result and before/after rating event per participant, with match ID, season, mode and rules version. Apply a match exactly once; retries cannot rate it again. Correction appends an auditable event and recomputes affected standings in ledger order. Show ranking, rating, provisional status, record, match history and season. A new season retains rating by default until a different published reset policy exists; earlier seasons stay queryable.

## Fairness and abuse

Publish map, vehicle eligibility, rating rules and result policy. Limit identity and queue abuse, detect suspicious collusion and provide an operator appeal/correction process; do not claim Elo prevents smurfing or manipulation. Every live fight and its rich debug information are publicly watchable by requirement, so an external observer can relay enemy information. The game API enforces seat-specific observations but cannot guarantee competitive secrecy against external information sources.

## Acceptance

Test atomic assignment and queue expiry, unique seats, both party and solo 2v2 formation, eligibility expansion, win/draw/abort decisions, rating examples against independently calculated results, duplicate result delivery, correction rebuild, separate mode/season standings, disconnects and recovery. Playtest vehicle/map asymmetry before assigning ranked eligibility.
