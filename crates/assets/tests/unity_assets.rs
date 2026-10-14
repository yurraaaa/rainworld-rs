//! The native Unity reader on the installed game (read-only), against AssetRipper
//! 2.0.0's export of the same files (`rainworld-re/tools/extract_atlases.sh`) where one
//! is on disk. Set `RW_DIR` (game install) and `RW_ASSETRIPPER_EXPORT` (an export's
//! `ExportedProject/Assets/Resources` directory) to override the defaults; tests skip
//! when the files are absent.

use std::path::{Path, PathBuf};

use rw_assets::unity::{AssetBundle, AudioClip, Texture2D, UnityObject};
use rw_assets::{NoiseTexture, PngImage, UnityResources};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";

/// The installed game's data directory and the AssetRipper export, when present.
struct Installed {
    data_dir: PathBuf,
    resources: UnityResources,
}

impl Installed {
    fn open() -> Option<Self> {
        let root =
            std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
        let data_dir = root.join("RainWorld_Data");
        if !data_dir.join("resources.assets").is_file() {
            eprintln!("skipping: no resources.assets under {}", data_dir.display());
            return None;
        }
        let resources = UnityResources::open(&data_dir).expect("resources open");
        Some(Self {
            data_dir,
            resources,
        })
    }

    /// The export's `Assets/Resources` directory: `RW_ASSETRIPPER_EXPORT`, else the
    /// copies in `~/.cache/rainworld-rs` or the last full export under `/tmp`.
    fn export(file: &str) -> Option<PathBuf> {
        let mut dirs: Vec<PathBuf> = std::env::var_os("RW_ASSETRIPPER_EXPORT")
            .map(PathBuf::from)
            .into_iter()
            .collect();
        if let Some(home) = std::env::var_os("HOME") {
            dirs.push(PathBuf::from(home).join(".cache/rainworld-rs"));
        }
        dirs.push(PathBuf::from(
            "/tmp/claude-1000/assetripper-slugcat/out/ExportedProject/Assets/Resources",
        ));
        let found = dirs.into_iter().map(|d| d.join(file)).find(|p| p.is_file());
        if found.is_none() {
            eprintln!("skipping comparison: no AssetRipper export of {file}");
        }
        found
    }

    fn bundle(&self, name: &str) -> Option<AssetBundle> {
        let path = self
            .data_dir
            .join("StreamingAssets/AssetBundles")
            .join(name);
        if !path.is_file() {
            eprintln!("skipping: no {}", path.display());
            return None;
        }
        Some(AssetBundle::open(&path).expect("bundle opens"))
    }
}

/// How two images differ: differing pixels, largest channel difference, first one.
#[derive(Debug, PartialEq, Eq)]
struct ImageDiff {
    size: (usize, usize),
    pixels: usize,
    max: u8,
    first: Option<(usize, usize, [u8; 4], [u8; 4])>,
}

impl ImageDiff {
    fn new(a: &PngImage, b: &PngImage) -> Self {
        assert_eq!((a.width(), a.height()), (b.width(), b.height()), "size");
        let mut diff = Self {
            size: (a.width(), a.height()),
            pixels: 0,
            max: 0,
            first: None,
        };
        for row in 0..a.height() {
            for x in 0..a.width() {
                let (p, q) = (a.pixel(x, row), b.pixel(x, row));
                if p != q {
                    diff.pixels += 1;
                    diff.first.get_or_insert((x, row, p, q));
                    let d = p.iter().zip(q).map(|(u, v)| u.abs_diff(v)).max().unwrap();
                    diff.max = diff.max.max(d);
                }
            }
        }
        diff
    }

    fn is_exact(&self) -> bool {
        self.pixels == 0
    }
}

/// FSB5 bank checks: magic, and header + sample headers + names + data = the size.
struct Fsb5<'a>(&'a [u8]);

impl Fsb5<'_> {
    fn u32(&self, at: usize) -> u64 {
        u64::from(u32::from_le_bytes(self.0[at..at + 4].try_into().unwrap()))
    }

    fn assert_whole(&self, what: &str) {
        assert_eq!(&self.0[..4], b"FSB5", "{what}");
        let total = 60 + self.u32(12) + self.u32(16) + self.u32(20);
        assert_eq!(total, self.0.len() as u64, "{what}: FSB5 sizes");
    }
}

#[test]
fn rain_world_atlas_matches_the_assetripper_export() {
    let Some(game) = Installed::open() else {
        return;
    };
    let texture: Texture2D = game.resources.load("Atlases/rainWorld").unwrap();
    assert_eq!((texture.width, texture.height), (464, 512));
    assert_eq!(texture.filter_mode, 0, "point");
    let image = game.resources.texture_image("Atlases/rainWorld").unwrap();
    let json = game.resources.text("atlases/RAINWORLD").unwrap();
    assert_eq!(json.len(), 120_262);
    if let Some(png) = Installed::export("atlases/rainWorld.png") {
        let export = PngImage::load(&png).unwrap();
        let diff = ImageDiff::new(&image, &export);
        assert!(diff.is_exact(), "pixel-exact: {diff:?}");
    }
    if let Some(path) = Installed::export("atlases/rainWorld.json") {
        assert_eq!(json, std::fs::read(path).unwrap());
    }
}

#[test]
fn every_texture_decodes_and_matches_its_export() {
    let Some(game) = Installed::open() else {
        return;
    };
    let mut paths: Vec<&str> = game.resources.paths().collect();
    paths.sort_unstable();
    let (mut decoded, mut compared) = (0, 0);
    for path in paths {
        let Ok(texture) = game.resources.load::<Texture2D>(path) else {
            continue;
        };
        let image = game.resources.texture_image(path).unwrap();
        decoded += 1;
        let dir = Path::new(path).parent().unwrap();
        let Some(png) = Installed::export(&format!("{}/{}.png", dir.display(), texture.name))
        else {
            continue;
        };
        let export = PngImage::load(&png).unwrap();
        let diff = ImageDiff::new(&image, &export);
        assert!(diff.is_exact(), "{path} ({:?}): {diff:?}", texture.format);
        compared += 1;
    }
    eprintln!("{decoded} textures decoded, {compared} compared with the export");
    assert_eq!(decoded, 39);
}

#[test]
fn noise_texture_equals_the_export_red_channel() {
    let Some(game) = Installed::open() else {
        return;
    };
    let noise = NoiseTexture::load(&game.resources).unwrap();
    let Some(png) = Installed::export("palettes/noise.png") else {
        return;
    };
    let export = NoiseTexture::from_image(&PngImage::load(&png).unwrap()).unwrap();
    let max = noise
        .bytes()
        .iter()
        .zip(export.bytes())
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap();
    assert_eq!(max, 0, "red channel equal to the export's");
}

#[test]
fn resources_audio_clips_are_fsb5_banks() {
    let Some(game) = Installed::open() else {
        return;
    };
    // Name, channels, frequency, length from AssetRipper's .audioclip export.
    for (path, channels, frequency, length) in [
        ("audio/old sfx unmixed/step1", 1, 48_000, 0.181_020_83),
        ("audio/old sfx unmixed/bassonly", 1, 44_100, 0.233_106_58),
    ] {
        let clip: AudioClip = game.resources.load(path).unwrap();
        assert_eq!(
            (clip.channels, clip.frequency),
            (channels, frequency),
            "{path}"
        );
        assert!((clip.length - length).abs() < 1e-6, "{path}");
        assert_eq!(clip.resource.path, "resources.resource");
        let data = game.resources.audio_data(&clip).unwrap();
        assert_eq!(data.len() as u64, clip.resource.size);
        Fsb5(&data).assert_whole(path);
    }
    let song: AudioClip = game.resources.load("music/procedural/su_1").unwrap();
    Fsb5(&game.resources.audio_data(&song).unwrap()).assert_whole("su_1");
}

#[test]
fn bundled_sound_effects_are_found_by_name() {
    let Some(game) = Installed::open() else {
        return;
    };
    let Some(bundle) = game.bundle("loadedsoundeffects") else {
        return;
    };
    assert_eq!(bundle.asset_paths().count(), 1619);
    assert!(bundle.contains("batchirp1a") && bundle.contains("Arc"));
    assert!(!bundle.contains("Arc_1"));
    // Sizes from `rainworld-re/tools/unity_assets.py bcontainer`.
    for (name, size, channels) in [
        ("Arc", 221_120, 2),
        ("bassOnly", 41_472, 2),
        ("batChirp1D", 211_808, 2),
        ("lizBite2E", 105_984, 2),
    ] {
        let clip: AudioClip = bundle.load(name).unwrap();
        assert_eq!(clip.name.to_lowercase(), name.to_lowercase());
        assert_eq!(
            (clip.resource.size, clip.channels),
            (size, channels),
            "{name}"
        );
        assert_eq!(clip.frequency, 44_100);
        let data = bundle.audio_data(&clip).unwrap();
        Fsb5(&data).assert_whole(name);
    }
    assert!(bundle.load::<AudioClip>("no such sound").is_err());
    // Every other audio bundle: clip count, and the first clip's bank is whole.
    for (name, count) in [
        ("loadedsoundeffects_ambient", 260),
        ("music_songs", 266),
        ("music_procedural", 262),
    ] {
        let Some(other) = game.bundle(name) else {
            continue;
        };
        let paths: Vec<&str> = other.asset_paths().collect();
        assert_eq!(paths.len(), count, "{name}");
        let clip: AudioClip = other.load(paths[0]).unwrap();
        Fsb5(&other.audio_data(&clip).unwrap()).assert_whole(paths[0]);
    }
    assert_eq!(AudioClip::CLASS_ID, 83);
}
