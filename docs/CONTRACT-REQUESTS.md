# Contract requests from the spectator viewer

Appended by the viewer owner. Each item is a field the spectator needs that
contract v1 does not carry. Until it lands, the viewer holds tanks at their
side's approach and shows defaults for the missing panels.

## Viewer requests, 2026-11-01

1. **Tank pose per snapshot**: `x_m`, `y_m` (sim plane), `heading_rad`,
   `turret_rad` (relative to hull), `gun_elevation_deg`, `speed_mps`.
   Please also state the axis convention. The viewer assumes sim +x is east
   and sim +y is north, and maps sim (x, y) to village (x, -y), because the
   village uses x east, z south, y up.
2. **Crew state per tank**: per role (commander, gunner, loader, driver,
   radio operator) one of ready, busy, wounded, out.
3. **Damage state per tank**: engine, left track, right track, turret ring,
   main gun as ok, damaged, destroyed; plus `on_fire` and `knocked_out`.
4. **Reload progress**: loader progress 0..1, or ticks remaining.
5. **Shot and impact events**: `shot {seat_id, round, from, to}` and
   `impact {target_seat_id?, at, effect}`.
6. **Seat metadata**: side (west or east), vehicle type, agent display name.
7. **Connection lifecycle events**: agent disconnected, agent reconnected,
   server recovery (with paused ticks), and a missing interval
   `{from_tick, to_tick}` when the recorded stream has a gap.
8. **Round type strings**: the full list of `ammunition` names
   (the viewer currently maps `ap`, `he`, `smoke`).
9. **Result detail**: winner seat or draw reason in the finished event, so the
   viewer does not have to infer it from `match_drawn`.
