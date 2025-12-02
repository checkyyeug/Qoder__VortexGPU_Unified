use crate::error::{FileIoError, VortexError};
use std::path::Path;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

/// Supported audio formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Wav,
    Flac,
    Mp3,
    Aac,
    Ogg,
    Opus,
    DsdDsf,
    DsdDff,
    Alac,
    Ape,
    WavPack,
    Unknown,
}

/// Format detector using magic numbers
pub struct FormatDetector;

impl FormatDetector {
    /// Detect audio format from file
    pub fn detect_format(path: &Path) -> Result<AudioFormat, VortexError> {
        let mut file = File::open(path)
            .map_err(|e| FileIoError::ReadError(e.to_string()))?;
        
        let mut magic_bytes = [0u8; 12];
        file.read_exact(&mut magic_bytes)
            .map_err(|e| FileIoError::ReadError(e.to_string()))?;
        
        // Check magic numbers
        let format = Self::detect_by_magic(&magic_bytes);
        
        if format != AudioFormat::Unknown {
            return Ok(format);
        }
        
        // Fallback to extension-based detection
        Self::detect_by_extension(path)
    }
    
    /// Detect format by magic number
    fn detect_by_magic(bytes: &[u8]) -> AudioFormat {
        // WAV: "RIFF....WAVE"
        if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
            return AudioFormat::Wav;
        }
        
        // FLAC: "fLaC"
        if bytes.len() >= 4 && &bytes[0..4] == b"fLaC" {
            return AudioFormat::Flac;
        }
        
        // MP3: "ID3" or FF FB/FF F3
        if bytes.len() >= 3 && &bytes[0..3] == b"ID3" {
            return AudioFormat::Mp3;
        }
        if bytes.len() >= 2 && bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0 {
            return AudioFormat::Mp3;
        }
        
        // OGG: "OggS"
        if bytes.len() >= 4 && &bytes[0..4] == b"OggS" {
            return AudioFormat::Ogg;
        }
        
        // DSD DSF: "DSD "
        if bytes.len() >= 4 && &bytes[0..4] == b"DSD " {
            return AudioFormat::DsdDsf;
        }
        
        // DSD DFF: "FRM8"
        if bytes.len() >= 4 && &bytes[0..4] == b"FRM8" {
            return AudioFormat::DsdDff;
        }
        
        // APE: "MAC "
        if bytes.len() >= 4 && &bytes[0..4] == b"MAC " {
            return AudioFormat::Ape;
        }
        
        AudioFormat::Unknown
    }
    
    /// Detect format by file extension
    fn detect_by_extension(path: &Path) -> Result<AudioFormat, VortexError> {
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .ok_or_else(|| FileIoError::UnsupportedFormat("No file extension".to_string()))?;
        
        match ext.as_str() {
            "wav" | "wave" => Ok(AudioFormat::Wav),
            "flac" => Ok(AudioFormat::Flac),
            "mp3" => Ok(AudioFormat::Mp3),
            "aac" | "m4a" => Ok(AudioFormat::Aac),
            "ogg" => Ok(AudioFormat::Ogg),
            "opus" => Ok(AudioFormat::Opus),
            "dsf" => Ok(AudioFormat::DsdDsf),
            "dff" => Ok(AudioFormat::DsdDff),
            "alac" => Ok(AudioFormat::Alac),
            "ape" => Ok(AudioFormat::Ape),
            "wv" => Ok(AudioFormat::WavPack),
            _ => Err(FileIoError::UnsupportedFormat(ext).into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_wav_magic() {
        let magic = b"RIFF\x00\x00\x00\x00WAVE";
        assert_eq!(FormatDetector::detect_by_magic(magic), AudioFormat::Wav);
    }
    
    #[test]
    fn test_flac_magic() {
        let magic = b"fLaC\x00\x00\x00\x00\x00\x00\x00\x00";
        assert_eq!(FormatDetector::detect_by_magic(magic), AudioFormat::Flac);
    }
    
    #[test]
    fn test_dsd_dsf_magic() {
        let magic = b"DSD \x00\x00\x00\x00\x00\x00\x00\x00";
        assert_eq!(FormatDetector::detect_by_magic(magic), AudioFormat::DsdDsf);
    }
    
    #[test]
    fn test_extension_detection() {
        let path = Path::new("test.flac");
        let result = FormatDetector::detect_by_extension(path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), AudioFormat::Flac);
    }
}
