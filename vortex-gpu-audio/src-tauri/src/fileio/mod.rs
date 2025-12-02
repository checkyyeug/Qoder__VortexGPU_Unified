// File I/O subsystem modules
pub mod loader;
pub mod format_detector;
pub mod metadata_extractor;
pub mod playlist_manager;

pub use loader::{AudioFileLoader, AudioData, AudioFileInfo};
pub use format_detector::{AudioFormat, FormatDetector};
pub use metadata_extractor::{AudioMetadata, MetadataExtractor};
pub use playlist_manager::{PlaylistManager, Playlist, PlaylistItem};
