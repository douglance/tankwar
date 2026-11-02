//! Spectator sound: engines and gunfire placed in the scene. It is played
//! only to people watching. Nothing here is sent anywhere, and agents have no
//! audio channel; every sound is derived from events they already received.

use crate::feed::EventKind;
use crate::tanks::TankView;
use crate::{Feeds, Playback, Screen, Viewing};
use bevy::audio::{AudioSource, PlaybackMode, Volume};
use bevy::prelude::*;
use std::sync::Arc;

pub struct AudioFxPlugin;

impl Plugin for AudioFxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, synth)
            .add_systems(Update, (attach_engines, engines, gunfire).after(Playback).run_if(in_state(Screen::Battlefield)));
    }
}

#[derive(Resource)]
struct Sounds {
    engine: Handle<AudioSource>,
    gun: Handle<AudioSource>,
}

#[derive(Component)]
struct Engine;

const RATE: u32 = 22050;

fn wav(samples: &[f32]) -> AudioSource {
    let data_len = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32000.0) as i16).to_le_bytes());
    }
    AudioSource { bytes: Arc::from(b.into_boxed_slice()) }
}

struct Noise(u32);
impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn synth(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let mut n = Noise(0x1234_5678);
    // Two-second diesel-ish idle loop: a low firing pulse plus filtered rumble.
    // 2 s at 27 Hz and 54 Hz are whole cycles, so the loop is seamless.
    let len = RATE as usize * 2;
    let mut lp = 0.0;
    let engine: Vec<f32> = (0..len)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            lp += (n.next() - lp) * 0.05;
            let pulse = (t * 27.0 * std::f32::consts::TAU).sin().max(0.0).powi(3);
            0.35 * pulse + 0.25 * (t * 54.0 * std::f32::consts::TAU).sin() + 0.5 * lp
        })
        .map(|s| s * 0.5)
        .collect();
    // Gun report: a sharp crack decaying into a low boom.
    let glen = (RATE as f32 * 1.6) as usize;
    let mut lp2 = 0.0;
    let gun: Vec<f32> = (0..glen)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let crack = n.next() * (-t * 40.0).exp();
            lp2 += (n.next() - lp2) * 0.02;
            let boom = (lp2 * 4.0 + (t * 48.0 * std::f32::consts::TAU).sin() * 0.4) * (-t * 3.0).exp();
            0.6 * crack + 0.7 * boom
        })
        .collect();
    commands.insert_resource(Sounds { engine: sources.add(wav(&engine)), gun: sources.add(wav(&gun)) });
}

fn attach_engines(mut commands: Commands, sounds: Res<Sounds>, tanks: Query<Entity, (With<TankView>, Without<Children>)>, with_kids: Query<(Entity, &Children), With<TankView>>, engines: Query<(), With<Engine>>) {
    let mut add = |e: Entity| {
        commands.entity(e).with_child((
            Engine,
            AudioPlayer(sounds.engine.clone()),
            PlaybackSettings { mode: PlaybackMode::Loop, volume: Volume::Linear(0.5), paused: true, spatial: true, ..default() },
            Transform::from_xyz(0.0, 1.0, -1.5),
        ));
    };
    for e in &tanks {
        add(e);
    }
    for (e, kids) in &with_kids {
        if !kids.iter().any(|k| engines.contains(k)) {
            add(e);
        }
    }
}

fn engines(feeds: Res<Feeds>, v: Res<Viewing>, tanks: Query<(&TankView, &Children)>, sinks: Query<&SpatialAudioSink, With<Engine>>) {
    let m = &feeds.0[v.idx];
    let halted = !v.live && v.paused;
    for (tv, kids) in &tanks {
        let on = m.tank_at(tv.0, v.cursor).map(|s| s.engine_on && !s.knocked_out).unwrap_or(false);
        for k in kids.iter() {
            if let Ok(sink) = sinks.get(k) {
                if on && !halted {
                    sink.play();
                } else {
                    sink.pause();
                }
            }
        }
    }
}

/// One gun report per shot crossed by the cursor during normal playback.
/// Seeking jumps silently.
fn gunfire(mut commands: Commands, sounds: Res<Sounds>, feeds: Res<Feeds>, v: Res<Viewing>) {
    let jump = (v.cursor - v.prev_cursor).abs();
    if v.cursor <= v.prev_cursor || jump > 20.0 * 4.0 {
        return;
    }
    let m = &feeds.0[v.idx];
    let a = m.events.partition_point(|e| (e.tick as f32) <= v.prev_cursor);
    let b = m.events_until(v.cursor);
    for e in &m.events[a..b] {
        if let EventKind::Shot { from, .. } = &e.kind {
            commands.spawn((
                AudioPlayer(sounds.gun.clone()),
                PlaybackSettings { mode: PlaybackMode::Despawn, volume: Volume::Linear(1.0), spatial: true, ..default() },
                Transform::from_translation(*from),
                StateScoped(Screen::Battlefield),
            ));
        }
    }
}
