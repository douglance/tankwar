# Content and vehicle specifications

Audience: historical researchers, content designers, technical artists and simulation engineers. Purpose: give each vehicle and scenario a repeatable, evidence-backed definition. Status: templates and initial candidate roster; values remain unknown until independently sourced.

## Roster policy

The long-term goal includes every WWII tank type. First define the catalog: countries, service dates, tank classes, prototypes versus operational service, vehicle families, variants, field modifications and theater-specific equipment. Record the inclusion rule and references before treating its denominator as exhaustive. A mesh named for a tank is not evidence of its exact variant or performance. Initial candidates are the supplied Panzer IV and Newc42's Panzer III J and Tiger I, pending variant research, asset review and the Panzer IV license. The downloaded Leichttraktor is excluded from the proposed release pending wartime eligibility evidence.

Coverage reports distinguish `cataloged`, `source gap`, `asset gap`, `implemented`, `tested` and `historically reviewed`. Do not equate one state with another. A model can only be enabled in a release when all required content and evidence for that release pass its gate.

## Vehicle design sheet template

| Field | Required record |
| --- | --- |
| Identity | Nation, family, exact model, production/configuration, theater and service date; inclusion rationale |
| Sources | Title, origin, date, page/section, stable link or archive ID, copyright status, competing evidence |
| Geometry | Overall dimensions, mass, track footprint, clearance, collision and armor geometry; units and confidence |
| Crew | Crew count, positions, overlapping roles, allowed reassignment and crew-task constraints |
| Mobility | Engine, transmission, gearing, speed, turn/terrain behavior, fuel capacity and consumption model |
| Observation | Each fitted sight/periscope/slit/hatch, field of view, magnification, viewing arc, exposure and limits |
| Armament | Main/secondary weapons, traverse/elevation limits and rates, firing sequence, reload and ammunition |
| Protection | Armor thickness/orientation, material assumptions, component placement, failure and damage model |
| Presentation | Exterior/interior, articulation, sound, effects, textures, rights, source and derived asset versions |
| Validation | Test fixture, independent expected value, source, tolerance, measurement result, reviewer and open uncertainty |

Each numeric field specifies units, supported configuration, source and uncertainty; a derived field documents its calculation. When sources disagree, retain both, record the decision and mark unresolved items. Never fill missing values with a neighboring tank's statistics without labeling and validating the approximation.

## Scenario design sheet template

Record scenario ID/version, title, fictional or documented setting, theater/date, map version and source references; allowed modes, teams and vehicle configurations; spawn positions and safe routes; match duration, win/draw/abort conditions; weather, lighting and terrain; ammunition and fuel starting loads; depot availability/stock/conditions; match seed; collision and destruction rules; spectator/replay requirements; known balance and historical limitations. The proposed initial scenario is a fictional rural village, 1v1/2v2, 15 minutes, no depot. Exact theater, date, dimensions and configurations are not yet established.

## Source and asset workflow

1. Identify the precise variant and acquire suitable primary documentation; log secondary interpretation and source conflicts separately.
2. Acquire exterior, crew views, sounds, environment and effect assets with documented licenses, creators, versions and hashes; compare them in an actual rendered scene.
3. Populate vehicle/scenario sheets, trace each behavior to evidence or an explicitly labeled approximation, and choose independent tolerances.
4. Test gameplay, visual alignment, visibility and replay versioning against the sheet; retain reviewer and result.
5. Approve for a named release only after the complete content gate passes. Reopen the record when an asset, source or gameplay model changes.
