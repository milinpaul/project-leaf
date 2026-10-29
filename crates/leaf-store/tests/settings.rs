//! App-wide settings: where they live, how they survive bad files, and their limits (E06-S02).

mod common;

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use common::TempDir;
use leaf_store::settings::{DEFAULT_SIDEBAR_WIDTH, SIDEBAR_WIDTH_RANGE, settings_path, visible_on};
use leaf_store::{AppSettings, Appearance, DisplayArea, WindowFrame};

fn frame(x: f32, y: f32, width: f32, height: f32) -> WindowFrame {
    WindowFrame {
        x,
        y,
        width,
        height,
        maximized: false,
    }
}

const LAPTOP: DisplayArea = DisplayArea {
    x: 0.0,
    y: 0.0,
    width: 1440.0,
    height: 900.0,
};

#[test]
fn settings_round_trip_through_the_file() {
    let dir = TempDir::new();
    let path = dir.0.join("nested").join("folder").join("settings.json");
    let mut settings = AppSettings::default();
    settings.set_window_frame(WindowFrame {
        maximized: true,
        ..frame(40.0, 30.0, 1100.0, 760.0)
    });
    settings.set_sidebar_width(300.0);
    settings.set_sidebar_hidden(true);
    settings.save(&path).unwrap();

    let loaded = AppSettings::load(&path);
    assert_eq!(loaded, settings);
    assert_eq!(loaded.sidebar_width(), 300.0);
    assert!(loaded.sidebar_hidden());
    assert!(loaded.window_frame(&[LAPTOP]).unwrap().maximized);
    assert!(
        !path.with_extension("json.tmp").exists(),
        "the temporary file is renamed away"
    );
}

#[test]
fn a_missing_file_gives_defaults() {
    let dir = TempDir::new();
    let settings = AppSettings::load(&dir.0.join("settings.json"));
    assert_eq!(settings, AppSettings::default());
    assert_eq!(settings.sidebar_width(), DEFAULT_SIDEBAR_WIDTH);
    assert!(!settings.sidebar_hidden());
    assert_eq!(settings.window_frame(&[LAPTOP]), None);
}

#[test]
fn fields_from_a_newer_leaf_are_kept() {
    let dir = TempDir::new();
    let path = dir.0.join("settings.json");
    fs::write(
        &path,
        r#"{"sidebar_width": 260, "recent": ["/a", "/b"], "future": {"x": 1}}"#,
    )
    .unwrap();

    let mut settings = AppSettings::load(&path);
    assert_eq!(settings.sidebar_width(), 260.0);
    settings.set_sidebar_width(270.0);
    settings.save(&path).unwrap();

    let saved: serde_json::Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved["recent"], serde_json::json!(["/a", "/b"]));
    assert_eq!(saved["future"]["x"], 1);
    assert_eq!(saved["sidebar_width"], 270.0);
}

#[test]
fn a_damaged_file_is_set_aside_and_defaults_are_used() {
    let dir = TempDir::new();
    let path = dir.0.join("settings.json");
    fs::write(&path, "{ not json").unwrap();

    assert_eq!(AppSettings::load(&path), AppSettings::default());
    assert!(!path.exists());
    assert_eq!(
        fs::read_to_string(dir.0.join("settings.json.bad")).unwrap(),
        "{ not json"
    );
}

#[test]
fn an_unreadable_path_gives_defaults() {
    let dir = TempDir::new();
    let folder = dir.0.join("settings.json");
    fs::create_dir_all(&folder).unwrap(); // a folder where the file should be
    assert_eq!(AppSettings::load(&folder), AppSettings::default());
    assert!(
        AppSettings::default().save(&folder).is_err(),
        "saving over a folder fails cleanly"
    );
}

#[test]
fn the_sidebar_width_stays_in_range() {
    let (min, max) = SIDEBAR_WIDTH_RANGE;
    let mut settings = AppSettings::default();
    for (asked, expected) in [
        (50.0, min),
        (10_000.0, max),
        (f32::NAN, DEFAULT_SIDEBAR_WIDTH),
        (333.0, 333.0),
    ] {
        settings.set_sidebar_width(asked);
        assert_eq!(settings.sidebar_width(), expected, "asked for {asked}");
    }
    let dir = TempDir::new();
    let path = dir.0.join("settings.json");
    fs::write(&path, r#"{"sidebar_width": 5}"#).unwrap();
    assert_eq!(
        AppSettings::load(&path).sidebar_width(),
        min,
        "an edited file is clamped too"
    );
}

#[test]
fn a_window_frame_is_restored_only_if_sensible_and_on_screen() {
    let mut settings = AppSettings::default();
    let cases = [
        (frame(100.0, 80.0, 1000.0, 700.0), true, "on screen"),
        (
            frame(3000.0, 80.0, 1000.0, 700.0),
            false,
            "on an unplugged monitor",
        ),
        (frame(100.0, 80.0, 200.0, 100.0), false, "too small"),
        (frame(f32::NAN, 80.0, 1000.0, 700.0), false, "not a number"),
        (frame(-950.0, 80.0, 1000.0, 700.0), false, "only a sliver visible"),
    ];
    for (saved, restored, why) in cases {
        settings.set_window_frame(saved);
        assert_eq!(settings.window_frame(&[LAPTOP]).is_some(), restored, "{why}");
    }
    let external = DisplayArea {
        x: 1440.0,
        y: 0.0,
        width: 2560.0,
        height: 1440.0,
    };
    settings.set_window_frame(frame(3000.0, 80.0, 1000.0, 700.0));
    assert!(
        settings.window_frame(&[LAPTOP, external]).is_some(),
        "on the second monitor"
    );
}

#[test]
fn with_no_displays_known_the_frame_is_trusted() {
    assert!(visible_on(&frame(5000.0, 5000.0, 800.0, 600.0), &[]));
}

#[test]
fn the_settings_file_lives_in_each_os_folder() {
    let env: HashMap<&str, &str> = [
        ("HOME", "/home/ana"),
        ("APPDATA", r"C:\Users\ana\AppData\Roaming"),
        ("XDG_CONFIG_HOME", "/cfg"),
    ]
    .into();
    let lookup = |name: &str| env.get(name).map(|v| (*v).to_owned());

    assert_eq!(
        settings_path("macos", lookup),
        Some(PathBuf::from(
            "/home/ana/Library/Application Support/Leaf/settings.json"
        ))
    );
    assert_eq!(
        settings_path("windows", lookup),
        Some(
            PathBuf::from(r"C:\Users\ana\AppData\Roaming")
                .join("Leaf")
                .join("settings.json")
        )
    );
    assert_eq!(
        settings_path("linux", lookup),
        Some(PathBuf::from("/cfg/leaf/settings.json"))
    );

    let home_only = |name: &str| (name == "HOME").then(|| "/home/ana".to_owned());
    assert_eq!(
        settings_path("linux", home_only),
        Some(PathBuf::from("/home/ana/.config/leaf/settings.json"))
    );
    let empty_xdg = |name: &str| match name {
        "HOME" => Some("/home/ana".to_owned()),
        "XDG_CONFIG_HOME" => Some(String::new()),
        _ => None,
    };
    assert_eq!(
        settings_path("freebsd", empty_xdg),
        Some(PathBuf::from("/home/ana/.config/leaf/settings.json"))
    );
    assert_eq!(settings_path("windows", home_only), None);
    assert_eq!(settings_path("macos", |_| None), None);
    assert!(leaf_store::settings::default_settings_path().is_some_and(|p| p.ends_with("settings.json")));
}

#[test]
fn the_appearance_choice_is_saved_and_unknown_values_follow_the_system() {
    let dir = TempDir::new();
    let path = dir.0.join("settings.json");
    let mut settings = AppSettings::default();
    assert_eq!(settings.appearance(), Appearance::System);
    settings.set_appearance(Appearance::Dark);
    settings.save(&path).unwrap();
    assert!(
        fs::read_to_string(&path)
            .unwrap()
            .contains(r#""appearance": "dark""#)
    );
    assert_eq!(AppSettings::load(&path).appearance(), Appearance::Dark);

    settings.set_appearance(Appearance::System);
    settings.save(&path).unwrap();
    assert!(
        !fs::read_to_string(&path).unwrap().contains("appearance"),
        "the default isn't written"
    );

    fs::write(&path, r#"{"appearance": "sepia", "sidebar_width": 300}"#).unwrap();
    let loaded = AppSettings::load(&path);
    assert_eq!(loaded.appearance(), Appearance::System);
    assert_eq!(loaded.sidebar_width(), 300.0, "the rest of the file still loads");
}
