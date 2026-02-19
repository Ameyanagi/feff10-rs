pub const DEFAULT_FILE_STATUS: &str = "UNKNOWN";
pub const DEFAULT_FILE_POSITION: &str = "REWIND";
pub const DEFAULT_FILE_ACTION: &str = "READWRITE";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenFlRequest {
    pub file_name: String,
    pub file_status: String,
    pub file_position: String,
    pub file_action: String,
}

pub fn default_openfl_request(file_name: impl Into<String>) -> OpenFlRequest {
    OpenFlRequest {
        file_name: file_name.into(),
        file_status: DEFAULT_FILE_STATUS.to_string(),
        file_position: DEFAULT_FILE_POSITION.to_string(),
        file_action: DEFAULT_FILE_ACTION.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_FILE_ACTION, DEFAULT_FILE_POSITION, DEFAULT_FILE_STATUS, default_openfl_request,
    };

    #[test]
    fn defaults_match_iomod_openfl_defaults() {
        let request = default_openfl_request("file1");
        assert_eq!(request.file_name, "file1");
        assert_eq!(request.file_status, DEFAULT_FILE_STATUS);
        assert_eq!(request.file_position, DEFAULT_FILE_POSITION);
        assert_eq!(request.file_action, DEFAULT_FILE_ACTION);
    }
}
