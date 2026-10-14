//! FSB5 decoding on the installed game's audio bundles (read-only). Set `RW_DIR` to
//! override the install; tests skip when the files are absent.

use std::collections::BTreeMap;
use std::path::PathBuf;

use rw_assets::unity::{AssetBundle, AudioClip, Fsb5, Fsb5Mode, VorbisSetups};

const DEFAULT_RW_DIR: &str = "/home/yura/.local/share/Steam/steamapps/common/Rain World";
const BUNDLES: [&str; 4] = [
    "loadedsoundeffects",
    "loadedsoundeffects_ambient",
    "music_songs",
    "music_procedural",
];

struct Bundles;

impl Bundles {
    fn open(name: &str) -> Option<AssetBundle> {
        let root =
            std::env::var_os("RW_DIR").map_or_else(|| PathBuf::from(DEFAULT_RW_DIR), PathBuf::from);
        let path = root
            .join("RainWorld_Data/StreamingAssets/AssetBundles")
            .join(name);
        if !path.is_file() {
            eprintln!("skipping: no {}", path.display());
            return None;
        }
        Some(AssetBundle::open(&path).expect("bundle opens"))
    }
}

/// Every clip of the sound-effect bundle: the bank parses, its header agrees with the
/// `AudioClip` metadata, and every Vorbis clip decodes to its full length.
#[test]
fn every_sound_effect_clip_decodes() {
    Bundles::check(&BUNDLES[..1], 3);
}

/// The same over all four bundles, ambience and music included (about 5 minutes; run with
/// `--ignored`).
#[test]
#[ignore = "decodes 830 MB of Vorbis"]
fn every_clip_of_every_audio_bundle_decodes() {
    Bundles::check(&BUNDLES, 26);
}

impl Bundles {
    /// Checks every clip of `bundles` and the number of distinct setup CRCs they use.
    fn check(bundles: &[&str], crc_count: usize) {
        let mut crcs: BTreeMap<u32, usize> = BTreeMap::new();
        for name in bundles {
            let Some(bundle) = Bundles::open(name) else {
                return;
            };
            let paths: Vec<String> = bundle.asset_paths().map(str::to_owned).collect();
            let (mut pcm, mut vorbis) = (0, 0);
            for path in &paths {
                let clip: AudioClip = bundle.load(path).unwrap();
                let data = bundle.audio_data(&clip).unwrap();
                let fsb = Fsb5::parse(&data).unwrap_or_else(|e| panic!("{path}: {e}"));
                assert_eq!(u32::from(fsb.channels), clip.channels, "{path}");
                assert_eq!(fsb.sample_rate, clip.frequency, "{path}");
                // m_Length is frames / frequency as a float.
                let length = fsb.frames as f32 / fsb.sample_rate as f32;
                assert!(
                    (length - clip.length).abs() < 1e-4,
                    "{path}: {length} vs {}",
                    clip.length
                );
                match fsb.mode {
                    Fsb5Mode::Pcm16 => {
                        assert_eq!(clip.compression_format, 0, "{path}");
                        pcm += 1;
                    }
                    Fsb5Mode::Vorbis => {
                        assert_eq!(clip.compression_format, 1, "{path}");
                        *crcs.entry(fsb.vorbis_crc.unwrap()).or_default() += 1;
                        let audio = fsb.decode().unwrap_or_else(|e| panic!("{path}: {e}"));
                        assert_eq!(audio.frames(), fsb.frames as usize, "{path}");
                        vorbis += 1;
                    }
                    Fsb5Mode::Other(m) => panic!("{path}: mode {m}"),
                }
            }
            eprintln!(
                "{name}: {} clips, {pcm} PCM16, {vorbis} Vorbis decoded",
                paths.len()
            );
        }
        let list: Vec<String> = crcs.iter().map(|(c, n)| format!("{c:08x}:{n}")).collect();
        eprintln!("{} setup CRCs: {}", crcs.len(), list.join(","));
        assert_eq!(crcs.len(), crc_count);
        for crc in crcs.keys() {
            assert!(VorbisSetups::get(*crc).is_some(), "{crc:08x} shipped");
        }
    }
}

/// `walk3A`, a footstep: mono or stereo PCM16 of 0.25 s at 44.1 kHz (sound.md 3).
#[test]
fn walk3a_decodes_to_its_length() {
    let Some(bundle) = Bundles::open(BUNDLES[0]) else {
        return;
    };
    let clip: AudioClip = bundle.load("walk3A").unwrap();
    let audio = bundle.decode_audio(&clip).unwrap();
    assert_eq!((audio.sample_rate, audio.frames()), (44_100, 11_025));
    assert_eq!(u32::from(audio.channels), clip.channels);
    assert!(audio.samples.iter().any(|&s| s != 0));
}
