# Economy, progression and balance design

Audience: game designers, product and competitive operators. Purpose: explain rewards, resources, unlocks and the balance policy. Status: initial design; no commercial model or pricing has been selected.

## Game economy

Ammunition, loaded rounds, fuel and scenario depot stock are in-match resources with physical limits. Their capacities and consumption come from the specific vehicle and scenario; the detailed handling rules are in [simulation and crew](02-simulation-and-crew.md). The initial ranked scenario has no mid-match depot. There is no designed persistent currency, store, paid advantage, vehicle upgrade tree or loot box; adding any would be a separate product decision and must not be assumed by the interface.

## Progression

The initial persistent progression is agent identity, separate 1v1/2v2 Elo, provisional status, season standings and searchable match history. Vehicles are available according to published release and ranked eligibility, not unlocked through an invented experience grind. Additional vehicles expand the available catalog after content gates pass. No progression alters a vehicle's historic specifications or secretly improves an agent's performance.

## Balance policy

Fair competition must coexist with differing historic vehicles. Proposed tools are compatible matchmaking, scenario and map selection, published vehicle eligibility, mirrored starts and transparent competition rules. Do not silently alter sourced armor, sight limits, speed, loadout or firing parameters to equalize win rates; any deliberate abstraction must be labeled and versioned. Public live viewing and debug telemetry can be relayed externally and therefore limit hidden-information fairness.

After adequate playtests, analyze outcomes by vehicle/configuration, opponent, map, spawn, mode and rating bracket, including sample size and confidence. Rating changes can be influenced by agent skill and selection; raw win rate alone does not establish vehicle imbalance. Record simulation version and rules before comparing cohorts. Define formal minimum samples and intervention thresholds before making competitive balance claims; they are not known yet.

## Acceptance

Verify conservation of fuel and ammunition, transparency of eligibility and resource rules, no rating effect from an aborted match, separate mode/season ratings and no undocumented statistical changes between versioned configurations. Test both teams and spawn sides for each ranked map. No commercial or unlock system is implied by the absence of one in this draft.
