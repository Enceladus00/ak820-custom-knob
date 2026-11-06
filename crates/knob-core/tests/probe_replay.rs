//! Replays a real recording of the AK820 MAX (2.4 GHz dongle) through the
//! engine and checks that every step of the guided probe test is understood
//! correctly. The fixture is an excerpt of a knob-probe log.

use knob_core::{Action, Config, Dir, Engine, Gesture, HookKey, Input, Micros, Modifiers, Output};

const FIXTURE: &str = include_str!("fixtures/ak820max-2.4g.log");
const AK820_CONSUMER: &str = "VID_0C45&PID_FEFC&MI_01&COL03";

/// Inputs for one step of the guided test.
#[derive(Default)]
struct Step {
    inputs: Vec<Input>,
}

/// Parses the fixture into steps (index = step number; 0 = before the test).
fn parse_steps() -> Vec<Step> {
    let mut steps: Vec<Step> = vec![Step::default()];
    let mut mods = Modifiers::default();

    for line in FIXTURE.lines() {
        if let Some(rest) = line.strip_prefix("==================== STEP ") {
            let number: usize = rest.split('/').next().unwrap().parse().unwrap();
            steps.resize_with(number + 1, Step::default);
            continue;
        }
        let Some((time, rest)) = line.strip_prefix('[').and_then(|l| l.split_once(']')) else {
            continue;
        };
        let t = (time.trim().parse::<f64>().unwrap() * 1000.0) as Micros;
        let rest = rest.trim_start();
        let inputs = &mut steps.last_mut().unwrap().inputs;

        if let Some(raw) = rest.strip_prefix("RAWHID") {
            if !raw.contains(AK820_CONSUMER) {
                continue;
            }
            let bytes = raw
                .split("bytes=[")
                .nth(1)
                .unwrap()
                .split(']')
                .next()
                .unwrap();
            let bytes: Vec<u8> = bytes
                .split(' ')
                .map(|b| u8::from_str_radix(b, 16).unwrap())
                .collect();
            let usage = u16::from_le_bytes([bytes[1], bytes[2]]);
            inputs.push(Input::DeviceReport { t, usage });
        } else if let Some(hook) = rest.strip_prefix("HOOK") {
            let mut words = hook.split_whitespace();
            let down = words.next() == Some("down");
            let vk =
                u32::from_str_radix(words.next().unwrap().trim_start_matches("vk=0x"), 16).unwrap();
            match vk {
                0xA2 | 0xA3 => mods.ctrl = down,
                0xA0 | 0xA1 => mods.shift = down,
                0xA4 | 0xA5 => mods.alt = down,
                0x5B | 0x5C => mods.win = down,
                _ => {
                    if let Some(key) = HookKey::from_vk(vk) {
                        inputs.push(Input::Key { t, key, down, mods });
                    }
                }
            }
        }
    }
    steps
}

/// Runs one step through the engine and returns everything it produced.
fn run_step(engine: &mut Engine, step: &Step) -> Vec<Output> {
    let mut out = Vec::new();
    for &input in &step.inputs {
        out.extend(engine.handle(input));
    }
    if let Some(last) = step.inputs.last() {
        out.extend(engine.poll(last.time() + 2_000_000));
    }
    out
}

fn engine() -> Engine {
    let mut engine = Engine::new(Config::default_config());
    engine.set_foreground_app("explorer.exe"); // Default profile, "volume" mode
    engine
}

fn rotations(out: &[Output], dir: Dir) -> usize {
    out.iter()
        .filter(|o| matches!(o, Output::Run { action: Action::Volume { .. }, gesture: Gesture::Rotate { dir: d, .. } } if *d == dir))
        .count()
}

fn mutes(out: &[Output]) -> usize {
    out.iter()
        .filter(|o| {
            matches!(
                o,
                Output::Run {
                    action: Action::Mute,
                    gesture: Gesture::Click { .. }
                }
            )
        })
        .count()
}

fn replays(out: &[Output], key: HookKey) -> usize {
    out.iter()
        .filter(|o| matches!(o, Output::Replay { key: k, down: true } if *k == key))
        .count()
}

fn key_downs(step: &Step, key: HookKey) -> usize {
    step.inputs
        .iter()
        .filter(|i| matches!(i, Input::Key { key: k, down: true, .. } if *k == key))
        .count()
}

#[test]
fn guided_probe_session_is_classified_correctly() {
    let steps = parse_steps();
    assert_eq!(steps.len(), 12, "fixture should contain steps 1..=11");
    let mut engine = engine();
    let mut results = Vec::new();
    for step in &steps {
        results.push(run_step(&mut engine, step));
    }

    // 1: clockwise, slowly, 5 clicks
    assert_eq!(rotations(&results[1], Dir::Cw), 5);
    assert_eq!(results[1].len(), 5, "{:#?}", results[1]);

    // 2: counter-clockwise, slowly, 5 clicks
    assert_eq!(rotations(&results[2], Dir::Ccw), 5);
    assert_eq!(results[2].len(), 5, "{:#?}", results[2]);

    // 3: fast spin: every detent is a rotation, nothing leaks through
    let detents = key_downs(&steps[3], HookKey::VolumeUp);
    assert!(
        detents > 30,
        "fast spin should have many detents, got {detents}"
    );
    assert_eq!(rotations(&results[3], Dir::Cw), detents);
    assert_eq!(results[3].len(), detents);

    // 4: knob pressed 3 times → 3 clicks (mute in volume mode)
    assert_eq!(mutes(&results[4]), 3);
    assert_eq!(results[4].len(), 3);

    // 5, 6: Fn+F3 / Fn+F2 are NOT the knob: replayed untouched
    assert_eq!(replays(&results[5], HookKey::VolumeUp), 3);
    assert!(
        results[5]
            .iter()
            .all(|o| matches!(o, Output::Replay { .. })),
        "{:#?}",
        results[5]
    );
    assert_eq!(replays(&results[6], HookKey::VolumeDown), 3);
    assert!(
        results[6]
            .iter()
            .all(|o| matches!(o, Output::Replay { .. })),
        "{:#?}",
        results[6]
    );

    // 7: the Fn mute key (Fn+F4) can't be told apart from a knob click (documented limitation)
    assert_eq!(mutes(&results[7]), 3);

    // 8: Fn + knob turns like the knob alone: 3 cw, 3 ccw
    assert_eq!(rotations(&results[8], Dir::Cw), 3);
    assert_eq!(rotations(&results[8], Dir::Ccw), 3);
    assert_eq!(results[8].len(), 6);

    // 9: Fn + knob press is a normal click
    assert_eq!(mutes(&results[9]), 1);
    assert_eq!(results[9].len(), 1);

    // 10: Ctrl + knob → the ctrl layer (focused app's volume)
    let app_volume = results[10]
        .iter()
        .filter(|o| matches!(o, Output::Run { action: Action::AppVolume { .. }, gesture: Gesture::Rotate { dir: Dir::Cw, mods, .. } } if mods.ctrl))
        .count();
    assert_eq!(app_volume, 3, "{:#?}", results[10]);
    assert_eq!(results[10].len(), 3);
}

#[test]
fn fast_spin_accelerates() {
    let steps = parse_steps();
    let mut engine = engine();
    let out = run_step(&mut engine, &steps[3]);
    let max_speed = out
        .iter()
        .filter_map(|o| match o {
            Output::Run {
                gesture: Gesture::Rotate { speed, .. },
                ..
            } => Some(*speed),
            _ => None,
        })
        .fold(0.0f32, f32::max);
    assert!(
        knob_core::accel_factor(max_speed) >= 3.0,
        "max speed {max_speed}"
    );

    // Fresh engine: time must not go backwards (step 1 was recorded before step 3).
    let slow = run_step(&mut self::engine(), &steps[1]);
    for o in slow {
        if let Output::Run {
            gesture: Gesture::Rotate { speed, .. },
            ..
        } = o
        {
            assert_eq!(
                knob_core::accel_factor(speed),
                1.0,
                "slow turning must not accelerate ({speed})"
            );
        }
    }
}
