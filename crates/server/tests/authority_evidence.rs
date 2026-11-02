use tankwar_protocol::{Command, Order, PROTOCOL_VERSION};
use tankwar_server::{ledger::Decision, match_runtime::DurableMatch, matchmaking::Assignment};
use tankwar_sim::competition::Mode;

fn running() -> DurableMatch {
    let a = Assignment { match_id:"authority".into(), mode:Mode::OneOnOne, first:vec!["alice".into()], second:vec!["bob".into()] };
    let mut g = DurableMatch::from_assignment(&a).unwrap();
    g.mark_ready("alice").unwrap(); g.mark_ready("bob").unwrap(); g.start(1000).unwrap(); g
}
fn command(id: &str, order: Order) -> Command {
    Command { protocol_version:PROTOCOL_VERSION, id:id.into(), match_id:"authority".into(), seat_id:"west-0".into(), expected_version:None, order }
}

#[test]
fn retry_conflict_and_disconnected_drive_expires_to_neutral() {
    let mut g = running();
    let engine = command("engine", Order::Engine { running:true });
    assert!(matches!(g.accept("alice",engine), Decision::Accept(_)));
    let drive = command("drive", Order::Drive { throttle:1000, steering:500, ttl_ticks:2 });
    let accepted = match g.accept("alice",drive.clone()) { Decision::Accept(event) => event, other => panic!("{other:?}") };
    assert_eq!(g.accept("alice",drive.clone()),Decision::Duplicate(accepted));
    let mut changed = drive; changed.order = Order::Drive { throttle:500, steering:500, ttl_ticks:2 };
    assert_eq!(g.accept("alice",changed),Decision::Reject("command_id_conflict"));
    assert_eq!(g.ledger.events.len(),2);
    g.advance(1050).unwrap();
    assert_eq!(g.world.vehicles["west-0"].tank.throttle,1000);
    assert_eq!(g.world.vehicles["west-0"].tank.steering,500);
    // No subsequent commands arrive: a dropped client cannot refresh its intent.
    g.advance(1100).unwrap();
    g.advance(1150).unwrap();
    let vehicle = &g.world.vehicles["west-0"];
    assert_eq!((vehicle.tank.throttle,vehicle.tank.steering),(0,0));
    let position = vehicle.position;
    g.advance(1200).unwrap();
    assert_eq!(g.world.vehicles["west-0"].position,position);
    assert_eq!(g.ledger.seats[0].agent_id,"alice");
}

#[test]
fn spectator_copy_cannot_change_authoritative_state_or_issue_orders() {
    let mut g = running();
    let original = g.world.vehicles["west-0"].position;
    let mut browser_snapshot = g.world.spectator_snapshot("authority",g.ledger.version);
    browser_snapshot.tanks.iter_mut().find(|t| t.seat_id == "west-0").unwrap().x_m = Some(9999.0);
    browser_snapshot.tick = 18000;
    assert_eq!(g.world.vehicles["west-0"].position,original);
    assert_eq!(g.world.tick,0);
    let forged = command("forged",Order::Engine{running:true});
    assert_eq!(g.accept("spectator",forged),Decision::Reject("unauthorized_seat"));
    assert_eq!(g.ledger.version,0);
    g.advance(1050).unwrap();
    assert_eq!(g.world.tick,1);
    assert_eq!(g.world.vehicles["west-0"].position,original);
}

#[test]
fn seat_sight_and_snapshot_are_server_owned() {
    use tankwar_protocol::MatchState;
    use tankwar_sim::geometry::Vec3;

    let mut g = running();
    g.world.structures.clear();
    g.world.obstacles.clear();
    for (seat, x) in [("west-0", 0.), ("east-0", 100.)] {
        let tank = g.world.vehicles.get_mut(seat).unwrap();
        tank.position = Vec3 { x, y: 0., z: 0. };
        tank.heading_radians = 0.;
    }
    assert_eq!(g.look("alice").unwrap().contacts.len(), 1);
    assert!(g.look("bob").unwrap().contacts.is_empty());
    assert_eq!(g.look("spectator"), Err("unauthorized_seat"));
    for (agent, seat) in [("alice", "west-0"), ("bob", "east-0")] {
        let view = g.observe_self(agent).unwrap();
        assert_eq!(view.tanks.len(), 1);
        assert_eq!(view.tanks[0].seat_id, seat);
        assert_eq!((view.tanks[0].x_m, view.tanks[0].y_m, view.tanks[0].heading_rad, view.tanks[0].team), (None, None, None, None));
        assert!(view.structures.is_empty());
    }
    assert_eq!(g.observe_self("spectator"), Err("unauthorized_seat"));
    println!("server sight: alice sees 1 contact; bob sees 0; spectator denied; each agent receives only its own redacted tank");

    let mut display = g.world.spectator_snapshot("authority", 0);
    display.tick = 18000;
    display.state = MatchState::Finished;
    display.tanks.iter_mut().find(|t| t.seat_id == "east-0").unwrap().knocked_out = Some(true);
    assert_eq!(g.world.tick, 0);
    assert_eq!(g.world.state, MatchState::Running);
    assert_eq!(g.world.vehicles["east-0"].hit_points, 1);
    assert!(g.finished.is_none());
    assert_eq!(g.look("alice").unwrap().contacts.len(), 1);
    assert!(g.look("bob").unwrap().contacts.is_empty());
    g.advance(1050).unwrap();
    assert_eq!(g.world.tick, 1);
    println!("altered spectator copy: authoritative tick=1, enemy hp=1, match running, seat visibility unchanged");
}

#[test]
fn combat_outcome_is_decided_by_server_not_spectator_or_wrong_seat() {
    use tankwar_protocol::MatchState;
    use tankwar_sim::{competition::Outcome, geometry::{Bounds, Vec3}, world::WorldEvent};

    for blocked in [true, false] {
        let mut g = running();
        // Arrange a loaded gun and a clear line except for the optional server-side wall.
        g.world.structures.clear();
        g.world.obstacles.clear();
        let west = g.world.vehicles.get_mut("west-0").unwrap();
        west.position = Vec3 { x:0., y:0., z:0. };
        west.heading_radians = 0.;
        west.tank.chamber = Some("ap".into());
        g.world.vehicles.get_mut("east-0").unwrap().position = Vec3 { x:20., y:0., z:0. };
        if blocked {
            g.world.obstacles.push(Bounds { min:Vec3{x:10.,y:-5.,z:-5.}, max:Vec3{x:11.,y:5.,z:5.} });
        }
        let mut display = g.world.spectator_snapshot("authority",0);
        display.state = MatchState::Finished;
        display.tanks.iter_mut().find(|t| t.seat_id == "east-0").unwrap().knocked_out = Some(true);
        let shot = command("fire",Order::Fire);
        assert_eq!(g.accept("spectator",shot.clone()),Decision::Reject("unauthorized_seat"));
        assert_eq!(g.accept("bob",shot.clone()),Decision::Reject("unauthorized_seat"));
        g.advance(1050).unwrap();
        assert_eq!(g.world.vehicles["east-0"].hit_points,1);
        assert_eq!(g.world.vehicles["west-0"].tank.chamber.as_deref(),Some("ap"));
        assert_eq!(g.world.state,MatchState::Running);
        assert!(g.finished.is_none());
        assert!(g.world_events.iter().all(|event| !matches!(event.event,WorldEvent::Shot { .. } | WorldEvent::Destroyed { .. } | WorldEvent::Finished { .. })));
        println!("wall={blocked}: spectator and wrong-seat fire rejected; no shot, damage, or result");

        assert!(matches!(g.accept("alice",shot),Decision::Accept(_)));
        g.advance(1100).unwrap();
        let trace = g.world_events.iter().find_map(|e| match &e.event { WorldEvent::Shot { trace:Some(trace), .. } => Some(trace), _ => None }).expect("server shot trace");
        if blocked {
            assert_eq!(trace.hit_kind,"wall");
            assert_eq!(g.world.vehicles["east-0"].hit_points,1);
            assert_eq!(g.world.state,MatchState::Running);
            assert!(g.finished.is_none());
            println!("authorized fire: server hit wall; enemy hp=1; match running");
        } else {
            assert_eq!(trace.hit_kind,"vehicle");
            assert_eq!(trace.target.as_deref(),Some("east-0"));
            assert_eq!(g.world.vehicles["east-0"].hit_points,0);
            assert!(g.world_events.iter().any(|event| event.event == WorldEvent::Destroyed { seat:"east-0".into() }));
            assert!(g.world_events.iter().any(|event| event.event == WorldEvent::Finished { winner:Some(0) }));
            assert_eq!(g.world.state,MatchState::Finished);
            assert_eq!(g.result().unwrap().3,Outcome::FirstWins);
            println!("authorized fire: server hit east-0; enemy hp=0; match finished winner=west");
        }
    }
}
