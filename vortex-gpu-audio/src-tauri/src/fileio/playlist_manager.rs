use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use crate::error::VortexError;
use uuid::Uuid;

/// Playlist item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistItem {
    pub id: String,
    pub path: PathBuf,
    pub title: String,
    pub duration_secs: f64,
}

/// Playlist
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub items: Vec<PlaylistItem>,
    pub current_index: Option<usize>,
}

impl Playlist {
    /// Create a new playlist
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            items: Vec::new(),
            current_index: None,
        }
    }
    
    /// Add item to playlist
    pub fn add_item(&mut self, item: PlaylistItem) {
        self.items.push(item);
    }
    
    /// Remove item by index
    pub fn remove_item(&mut self, index: usize) -> Option<PlaylistItem> {
        if index < self.items.len() {
            Some(self.items.remove(index))
        } else {
            None
        }
    }
    
    /// Get current item
    pub fn current_item(&self) -> Option<&PlaylistItem> {
        self.current_index.and_then(|idx| self.items.get(idx))
    }
}

/// Playlist manager with JSON persistence
pub struct PlaylistManager {
    playlists: Vec<Playlist>,
}

impl PlaylistManager {
    /// Create a new playlist manager
    pub fn new() -> Self {
        Self {
            playlists: Vec::new(),
        }
    }
    
    /// Create a new playlist
    pub fn create_playlist(&mut self, name: String) -> String {
        let playlist = Playlist::new(name);
        let id = playlist.id.clone();
        self.playlists.push(playlist);
        id
    }
    
    /// Get playlist by ID
    pub fn get_playlist(&self, id: &str) -> Option<&Playlist> {
        self.playlists.iter().find(|p| p.id == id)
    }
    
    /// Save playlists to JSON
    pub fn save_to_json(&self, path: &std::path::Path) -> Result<(), VortexError> {
        let json = serde_json::to_string_pretty(&self.playlists)
            .map_err(|e| crate::error::FileIoError::WriteError(e.to_string()))?;
        
        std::fs::write(path, json)
            .map_err(|e| crate::error::FileIoError::WriteError(e.to_string()))?;
        
        Ok(())
    }
    
    /// Load playlists from JSON
    pub fn load_from_json(path: &std::path::Path) -> Result<Self, VortexError> {
        let json = std::fs::read_to_string(path)
            .map_err(|e| crate::error::FileIoError::ReadError(e.to_string()))?;
        
        let playlists: Vec<Playlist> = serde_json::from_str(&json)
            .map_err(|e| crate::error::FileIoError::ParseError(e.to_string()))?;
        
        Ok(Self { playlists })
    }
}

impl Default for PlaylistManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_playlist_creation() {
        let playlist = Playlist::new("My Playlist".to_string());
        assert_eq!(playlist.name, "My Playlist");
        assert_eq!(playlist.items.len(), 0);
    }
    
    #[test]
    fn test_add_item() {
        let mut playlist = Playlist::new("Test".to_string());
        let item = PlaylistItem {
            id: Uuid::new_v4().to_string(),
            path: PathBuf::from("test.flac"),
            title: "Test Song".to_string(),
            duration_secs: 180.0,
        };
        
        playlist.add_item(item);
        assert_eq!(playlist.items.len(), 1);
    }
    
    #[test]
    fn test_playlist_manager() {
        let mut manager = PlaylistManager::new();
        let id = manager.create_playlist("Test Playlist".to_string());
        
        assert!(manager.get_playlist(&id).is_some());
    }
}
