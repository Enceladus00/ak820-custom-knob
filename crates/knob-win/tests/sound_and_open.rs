//! Checks against this PC's real sound devices and shell. They change the
//! microphone and the default output for a moment and put them back, so they
//! only run when asked: `cargo test -p knob-win --test sound_and_open -- --ignored --test-threads=1`

use knob_win::audio::Audio;
use knob_win::launch;

#[test]
#[ignore]
fn microphone_mute_and_volume_round_trip() {
    let audio = Audio::new().unwrap();
    let Some((level, muted)) = audio.mic_state().unwrap() else {
        eprintln!("no microphone on this PC");
        return;
    };
    eprintln!("microphone: {:.0}% muted={muted}", level * 100.0);
    assert_eq!(audio.toggle_mic_mute().unwrap(), Some(!muted));
    assert_eq!(audio.mic_state().unwrap().unwrap().1, !muted);
    assert_eq!(audio.toggle_mic_mute().unwrap(), Some(muted));
    assert_eq!(audio.mic_state().unwrap().unwrap().1, muted);

    let delta = if level > 0.9 { -0.05 } else { 0.05 };
    let changed = audio.change_mic_volume(delta).unwrap().unwrap();
    assert!(
        (changed - (level + delta)).abs() < 0.01,
        "{changed} vs {level}"
    );
    audio.set_mic_volume(level).unwrap();
    let back = audio.mic_state().unwrap().unwrap().0;
    assert!((back - level).abs() < 0.005, "{back} vs {level}");
}

#[test]
#[ignore]
fn output_switches_and_switches_back() {
    let audio = Audio::new().unwrap();
    let original = audio.output_name().unwrap();
    eprintln!("default output: {original}");
    let Some(next) = audio.switch_output(1).unwrap() else {
        eprintln!("only one output on this PC");
        return;
    };
    let now = audio.output_name().unwrap();
    // Put it back before checking anything else.
    let back = audio.switch_output(-1).unwrap();
    if audio.output_name().unwrap() != original {
        audio.set_output_by_name(&original).unwrap();
    }
    eprintln!("switched to: {next}, then back to {back:?}");
    assert_eq!(now, next);
    assert_eq!(back.as_deref(), Some(original.as_str()));
    assert_eq!(audio.output_name().unwrap(), original);
}

#[test]
#[ignore]
fn opens_an_app_and_reports_a_missing_one() {
    assert!(launch::open_now("notepad.exe", ""));
    assert!(!launch::open_now(r"C:\no such folder\nothing here.xyz", ""));
}
