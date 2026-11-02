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
