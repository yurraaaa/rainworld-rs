//! The installed game's superstructure projections (read-only): the rooms with a
//! `SuperStructureProjector` effect (4), a `ProjectedScanLines` effect (3) and a
//! `ProjectedStars` placed object (4) are counted from the settings files; each projector
//! and scan line set builds on its room's size and runs 100 ticks keeping its lists in step
//! with the ideal counts, each star matrix builds from its object's handle and runs 100
//! ticks with finite positions. Set `RW_DIR` to the game install directory; skipped if
//! absent.

use std::path::{Path, PathBuf};

use rw_assets::{ActiveMods, PlacedObject, PlacedObjectType, ResizableData, Room};
use rw_random::UnityRandom;
use rw_sim::{
    PartKind, ProjectedScanLines, ProjectorFrame, StarMatrix, SuperStructureProjector, Vec2,
};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

struct Install;

impl Install {
    fn streaming_assets() -> Option<PathBuf> {
        let root =
            std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
        let sa = root.join("RainWorld_Data").join("StreamingAssets");
        if sa.join("world").join("regions.txt").is_file() {
            Some(sa)
        } else {
            eprintln!("skipping: no world under {}", sa.display());
            None
        }
    }

    /// Every `*_settings*.txt` under the base regions' room folders, gates and arena
    /// levels.
    fn settings_files(sa: &Path) -> Vec<PathBuf> {
        let mut dirs = vec![sa.join("world").join("gates"), sa.join("levels")];
        let regions =
            std::fs::read_to_string(sa.join("world").join("regions.txt")).expect("regions.txt");
        for r in regions.lines().map(str::trim).filter(|r| !r.is_empty()) {
            dirs.push(sa.join("world").join(format!("{}-rooms", r.to_lowercase())));
        }
        let mut files: Vec<PathBuf> = dirs
            .iter()
            .filter_map(|d| std::fs::read_dir(d).ok())
            .flatten()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains("_settings"))
            })
            .collect();
        files.sort();
        files
    }

    /// The geometry file beside a settings file (`x_settings-white.txt` -> `x.txt`).
    fn geometry(settings: &Path) -> Option<Room> {
        let name = settings.file_name()?.to_str()?;
        let stem = &name[..name.find("_settings")?];
        Room::load(settings.with_file_name(format!("{stem}.txt"))).ok()
    }

    /// The amount of effect `kind` in a settings file's `Effects:` line.
    fn effect_amount(text: &str, kind: &str) -> Option<f32> {
        let line = text.lines().find_map(|l| l.strip_prefix("Effects: "))?;
        line.split(", ").find_map(|e| {
            let mut fields = e.split('-');
            (fields.next()? == kind)
                .then(|| fields.next()?.parse().ok())
                .flatten()
        })
    }
}

const FRAME: ProjectorFrame = ProjectorFrame {
    electric_power: 1.0,
    being_viewed: true,
    camera_position: None,
};

/// A room's update list in game order: newest first.
fn tick_projector(p: &mut SuperStructureProjector, seq: &mut u64, rng: &mut UnityRandom) {
    let mut orders: Vec<u64> = p
        .parts
        .iter()
        .filter(|q| !q.slated)
        .map(|q| q.update_order)
        .collect();
    orders.push(p.update_order);
    orders.sort_unstable_by(|a, b| b.cmp(a));
    for order in orders {
        if order == p.update_order {
            p.update(&FRAME, seq, rng);
        } else {
            p.update_part(order, seq, rng);
        }
    }
}

#[test]
fn projector_rooms_build_and_run() {
    let Some(sa) = Install::streaming_assets() else {
        return;
    };
    let mut rooms = Vec::new();
    for path in Install::settings_files(&sa) {
        let text = std::fs::read_to_string(&path).expect("settings");
        if let Some(amount) = Install::effect_amount(&text, "SuperStructureProjector") {
            rooms.push((path, amount));
        }
    }
    let names: Vec<String> = rooms
        .iter()
        .map(|(p, a)| format!("{} {a}", p.file_name().unwrap().to_string_lossy()))
        .collect();
    assert_eq!(rooms.len(), 4, "{names:?}");
    for (path, amount) in rooms {
        let room = Install::geometry(&path).expect("geometry");
        let tiles = (room.width(), room.height());
        let mut rng = UnityRandom::new(3);
        let mut seq = 0;
        let mut p = SuperStructureProjector::new(tiles, amount, &mut seq, &mut rng);
        assert_eq!(p.glyphs_list.len(), p.ideal_glyph_number() / 2);
        assert_eq!(p.scan_lines.len(), p.ideal_scan_line_number());
        for _ in 0..100 {
            tick_projector(&mut p, &mut seq, &mut rng);
        }
        assert!(!p.slated);
        assert!(p.glyphs_list.len() >= p.ideal_glyph_number().min(1));
        assert!(p.scan_lines.len() <= p.ideal_scan_line_number() + 2);
        assert!(p.cursors.len() <= p.ideal_cursor_number() + 1);
        assert!(p.grid_pos.x.abs() <= 16.0 + 5.0 && p.grid_pos.y.abs() <= 16.0 + 5.0);
        for part in p.parts.iter().filter(|q| !q.slated) {
            match part.kind {
                PartKind::Single(s) => assert!((0..95).contains(&s.glyph.pos.0)),
                PartKind::Matrix(m) => assert!(m.size.0 >= 1 && m.size.1 >= 1),
                PartKind::Cursor(c) => assert!(c.pos.x.is_finite() && c.pos.y.is_finite()),
                PartKind::ScanLine(l) => assert!(l.pos.is_finite()),
            }
        }
    }
}

#[test]
fn scan_line_rooms_build_and_run() {
    let Some(sa) = Install::streaming_assets() else {
        return;
    };
    let mut rooms = Vec::new();
    for path in Install::settings_files(&sa) {
        let text = std::fs::read_to_string(&path).expect("settings");
        if let Some(amount) = Install::effect_amount(&text, "ProjectedScanLines") {
            rooms.push((path, amount));
        }
    }
    assert_eq!(rooms.len(), 3, "{rooms:?}");
    for (path, amount) in rooms {
        let room = Install::geometry(&path).expect("geometry");
        let tiles = (room.width(), room.height());
        let mut rng = UnityRandom::new(4);
        let mut seq = 0;
        let mut p = ProjectedScanLines::new(tiles, amount, &mut seq, &mut rng);
        assert_eq!(p.lines.len(), p.ideal_scan_line_number());
        for _ in 0..100 {
            let mut orders: Vec<u64> = p.lines.iter().map(|l| l.update_order).collect();
            orders.push(p.update_order);
            orders.sort_unstable_by(|a, b| b.cmp(a));
            for order in orders {
                if order == p.update_order {
                    p.update(1.0, true, &mut seq, &mut rng);
                } else {
                    p.update_line(order);
                }
            }
            p.remove_slated();
        }
        assert!(!p.slated);
        assert!(p.scan_lines.len() >= p.ideal_scan_line_number().saturating_sub(1));
    }
}

#[test]
fn projected_stars_build_and_run() {
    let Some(sa) = Install::streaming_assets() else {
        return;
    };
    let mut found = Vec::new();
    for path in Install::settings_files(&sa) {
        let text = std::fs::read_to_string(&path).expect("settings");
        let Some(value) = text.lines().find_map(|l| l.strip_prefix("PlacedObjects: ")) else {
            continue;
        };
        for o in PlacedObject::parse_list(value) {
            if o.object_type(ActiveMods::NONE) == Some(PlacedObjectType::ProjectedStars) {
                found.push((path.clone(), o));
            }
        }
    }
    let rooms: std::collections::BTreeSet<String> = found
        .iter()
        .map(|(p, _)| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(rooms.len(), 4, "{rooms:?}");
    for (path, o) in found {
        let d = ResizableData::parse(&o.data).expect("data");
        let mut rng = UnityRandom::new(5);
        let mut m = StarMatrix::new(1, Vec2::from(o.pos), d.rad(), &mut rng);
        assert_eq!(m.rad, d.rad() + 50.0);
        assert!(!m.stars.is_empty(), "{}", path.display());
        for _ in 0..100 {
            m.update(&mut rng);
        }
        assert_eq!(m.z_rotat, 0.0);
        for i in 0..m.stars.len() {
            let at = m.on_plane_pos(i, 0.5);
            assert!(at.x.is_finite() && at.y.is_finite());
        }
    }
}
