//! A read-only reader for the game's Unity asset files: serialized files
//! (`resources.assets`, `globalgamemanagers`), their streamed data (`.resS`,
//! `.resource`) and UnityFS bundles (`StreamingAssets/AssetBundles`). Format:
//! `rainworld-re/notes/unity-assets.md`.

mod bcn;
mod bundle;
mod objects;
mod reader;
mod resources;
mod serialized;

pub use bundle::{AssetBundle, BundleNode};
pub use objects::{AudioClip, ImageData, StreamedResource, TextAsset, Texture2D, TextureFormat};
pub use resources::UnityResources;
pub use serialized::{ObjectInfo, SerializedFile, UnityObject};
