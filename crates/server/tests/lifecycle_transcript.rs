use tankwar_protocol::{Command, MatchState, Order, PROTOCOL_VERSION, MATCH_TICKS};
use tankwar_server::{ledger::Decision, match_runtime::DurableMatch, matchmaking::Queue};
use tankwar_sim::competition::{Mode, Ratings};

fn assigned(queue: &mut Queue, mode: Mode, agents: &[&str]) -> DurableMatch {
    for agent in &agents[..agents.len()-1] {
        assert!(queue.join(agent, mode).unwrap().is_none());
        assert!(agents.iter().all(|a| queue.assignment(a).is_none()));
    }
    println!("queued mode={mode:?} agents={agents:?} bound=0");
    let assignment = queue.join(agents[agents.len()-1], mode).unwrap().unwrap();
    for agent in agents { assert_eq!(queue.assignment(agent), Some(&assignment)); }
    let queue_copy: Queue = serde_json::from_slice(&serde_json::to_vec(queue).unwrap()).unwrap();
    for agent in agents { assert_eq!(queue_copy.assignment(agent), Some(&assignment)); }
    let game = DurableMatch::from_assignment(&assignment).unwrap();
    assert_eq!(game.ledger.state, MatchState::Assigned);
    assert_eq!(game.ledger.seats.len(), agents.len());
    println!("assigned match={} seats={:?}", assignment.match_id, game.ledger.seats);
    game
}
fn start(game: &mut DurableMatch) {
    let agents: Vec<_> = game.ledger.seats.iter().map(|s| s.agent_id.clone()).collect();
    for (i, agent) in agents.iter().enumerate() { assert_eq!(game.mark_ready(agent).unwrap(), i+1 == agents.len()); }
    assert_eq!(game.ledger.state, MatchState::Ready);
    println!("ready match={}",game.ledger.match_id);
    game.start(1000).unwrap();
    assert_eq!(game.world.state, MatchState::Running);
    println!("running match={}",game.ledger.match_id);
}
fn finish(game: &mut DurableMatch, win: bool) {
    if win { for vehicle in game.world.vehicles.values_mut() { if vehicle.team == 1 { vehicle.hit_points = 0; } } }
    else { game.world.tick = MATCH_TICKS - 1; }
    game.advance(1050).unwrap();
    assert_eq!(game.world.state, MatchState::Finished);
    println!("finished match={} winner={:?} tick={} (fixture: {})",game.ledger.match_id,game.finished.as_ref().unwrap().winner,game.world.tick, if win {"opponents destroyed"} else {"clock set to tick 17999"});
}
#[test]
fn lifecycle_and_ranked_runtime_transcript() {
    let mut queue = Queue::default();
    let mut ratings = Ratings::for_season("season-1").unwrap();
    let mut one = assigned(&mut queue, Mode::OneOnOne, &["alice", "bob"]);
    start(&mut one);
    let seat = one.ledger.seats[0].seat_id.clone();
    let saved = serde_json::to_vec(&one).unwrap(); // Simulate loss of the client and actor reload, not a network disconnect.
    drop(one);
    let mut one: DurableMatch = serde_json::from_slice(&saved).unwrap();
    let snapshot = one.observe_self("alice").unwrap();
    assert_eq!(snapshot.tanks.len(),1); assert_eq!(snapshot.tanks[0].seat_id, seat);
    let command = Command { protocol_version:PROTOCOL_VERSION, id:"reconnected".into(), match_id:one.ledger.match_id.clone(), seat_id:seat.clone(), expected_version:None, order:Order::Engine{running:true} };
    assert!(matches!(one.accept("alice",command),Decision::Accept(_)));
    println!("client dropped; persisted match reloaded; alice reuses seat={seat}; command accepted");
    finish(&mut one,true);
    let changes = one.rate_finished(&mut ratings,Mode::OneOnOne).unwrap();
    assert_eq!(changes[0].before,1500.); assert_eq!(changes[0].after,1516.); assert_eq!(changes[1].after,1484.);
    println!("1v1 K=32 {changes:?}");
    one.rate_finished(&mut ratings,Mode::OneOnOne).unwrap();
    assert_eq!(ratings.games(Mode::OneOnOne,"alice"),1); assert_eq!(ratings.rating(Mode::OneOnOne,"alice"),1516.);
    println!("1v1 duplicate match={} games=1 rating=1516",one.ledger.match_id);
    queue.release(&one.ledger.match_id);
    for i in 2..=10 {
        let mut game=assigned(&mut queue,Mode::OneOnOne,&["alice","bob"]); start(&mut game); finish(&mut game,false);
        game.rate_finished(&mut ratings,Mode::OneOnOne).unwrap(); queue.release(&game.ledger.match_id);
        if i == 9 || i == 10 { println!("1v1 alice games={i} provisional={} rating={:.6}", ratings.provisional(Mode::OneOnOne,"alice"),ratings.rating(Mode::OneOnOne,"alice")); }
        assert_eq!(ratings.provisional(Mode::OneOnOne,"alice"),i<10);
    }
    let mut warm=assigned(&mut queue,Mode::TwoOnTwo,&["carol","partner","x","y"]); start(&mut warm); finish(&mut warm,true); warm.rate_finished(&mut ratings,Mode::TwoOnTwo).unwrap(); queue.release(&warm.ledger.match_id);
    let mut two=assigned(&mut queue,Mode::TwoOnTwo,&["alice","bob","carol","dave"]); start(&mut two); finish(&mut two,true);
    let changes=two.rate_finished(&mut ratings,Mode::TwoOnTwo).unwrap();
    let expected=1500.0+32.0*(1.0-1.0/(1.0+10_f64.powf((1508.0-1500.0)/400.0)));
    assert!((changes[0].after-expected).abs()<1e-9); assert_eq!(changes[2].before,1516.); assert_eq!(changes[3].before,1500.);
    println!("2v2 opponent mean=(1516+1500)/2=1508 K=32 changes={changes:?}");
    two.rate_finished(&mut ratings,Mode::TwoOnTwo).unwrap(); assert_eq!(ratings.games(Mode::TwoOnTwo,"alice"),1);
    println!("2v2 duplicate match={} games=1 rating={:.6}; 1v1 games={}",two.ledger.match_id,ratings.rating(Mode::TwoOnTwo,"alice"),ratings.games(Mode::OneOnOne,"alice"));
    queue.release(&two.ledger.match_id);
    let mut cancelled=assigned(&mut queue,Mode::OneOnOne,&["cancel-a","cancel-b"]); cancelled.cancel_unready().unwrap(); assert_eq!(cancelled.world.state,MatchState::Cancelled); println!("cancelled match={} reason=ready_timeout",cancelled.ledger.match_id); queue.release(&cancelled.ledger.match_id);
    let mut aborted=assigned(&mut queue,Mode::OneOnOne,&["abort-a","abort-b"]); start(&mut aborted);
    assert!(aborted.advance(1_000_000).is_err()); assert_eq!(aborted.world.state,MatchState::Aborted);
    let before=serde_json::to_vec(&ratings).unwrap(); assert!(aborted.rate_finished(&mut ratings,Mode::OneOnOne).is_err()); assert_eq!(serde_json::to_vec(&ratings).unwrap(),before);
    println!("aborted match={} rating unchanged; result={:?}",aborted.ledger.match_id,aborted.rating_result().err());
}
