#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IoFile {
    pub file_name: String,
    pub unit_number: i32,
    pub n_sections: usize,
    pub file_action: String,
    pub eof: bool,
}

impl IoFile {
    fn new(file_name: &str) -> Self {
        Self {
            file_name: file_name.to_string(),
            unit_number: 0,
            n_sections: 0,
            file_action: "READWRITE".to_string(),
            eof: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IoFileInfoUpdate {
    pub unit_number: Option<i32>,
    pub n_sections: Option<usize>,
    pub file_action: Option<String>,
    pub eof: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IoFileStack {
    files: Vec<IoFile>,
}

impl IoFileStack {
    pub fn add_io_file(&mut self, file_name: &str) -> bool {
        if self.index_io_file(file_name).is_some() {
            return false;
        }

        self.files.push(IoFile::new(file_name));
        true
    }

    pub fn delete_io_file(&mut self, file_name: &str) -> bool {
        if file_name.eq_ignore_ascii_case("ALL") {
            let had_files = !self.files.is_empty();
            self.files.clear();
            return had_files;
        }

        let Some(index) = self
            .files
            .iter()
            .position(|entry| entry.file_name.eq_ignore_ascii_case(file_name))
        else {
            return false;
        };

        self.files.remove(index);
        true
    }

    pub fn index_io_file(&self, file_name: &str) -> Option<usize> {
        self.files
            .iter()
            .position(|entry| entry.file_name.eq_ignore_ascii_case(file_name))
            .map(|index| index + 1)
    }

    pub fn set_io_file_info(&mut self, file_name: &str, update: IoFileInfoUpdate) -> bool {
        let Some(entry) = self
            .files
            .iter_mut()
            .find(|entry| entry.file_name.eq_ignore_ascii_case(file_name))
        else {
            return false;
        };

        if let Some(unit_number) = update.unit_number {
            entry.unit_number = unit_number;
        }
        if let Some(n_sections) = update.n_sections {
            entry.n_sections = n_sections;
        }
        if let Some(file_action) = update.file_action {
            entry.file_action = file_action;
        }
        if let Some(eof) = update.eof {
            entry.eof = eof;
        }

        true
    }

    pub fn files(&self) -> &[IoFile] {
        &self.files
    }
}

pub fn run_testiofiles_scenario() -> IoFileStack {
    let mut stack = IoFileStack::default();

    stack.add_io_file("file1");
    stack.add_io_file("file2");
    stack.add_io_file("file3");

    stack.delete_io_file("file2");
    stack.add_io_file("file2");

    stack.add_io_file("file1");

    stack.set_io_file_info(
        "file1",
        IoFileInfoUpdate {
            unit_number: Some(1),
            n_sections: Some(1),
            file_action: Some("READWRITE".to_string()),
            eof: Some(true),
        },
    );

    stack.delete_io_file("file2");
    stack.delete_io_file("file3");
    stack
}

#[cfg(test)]
mod tests {
    use super::{IoFileInfoUpdate, IoFileStack, run_testiofiles_scenario};

    #[test]
    fn stack_uses_one_based_indexing() {
        let mut stack = IoFileStack::default();
        stack.add_io_file("file1");
        stack.add_io_file("file2");
        assert_eq!(stack.index_io_file("file1"), Some(1));
        assert_eq!(stack.index_io_file("file2"), Some(2));
    }

    #[test]
    fn duplicate_add_is_ignored_like_legacy_iofiles() {
        let mut stack = IoFileStack::default();
        assert!(stack.add_io_file("file1"));
        assert!(!stack.add_io_file("file1"));
        assert_eq!(stack.files().len(), 1);
    }

    #[test]
    fn set_io_file_info_updates_selected_fields() {
        let mut stack = IoFileStack::default();
        stack.add_io_file("file1");
        let updated = stack.set_io_file_info(
            "file1",
            IoFileInfoUpdate {
                unit_number: Some(17),
                n_sections: Some(2),
                file_action: Some("READ".to_string()),
                eof: Some(true),
            },
        );
        assert!(updated);

        let file = &stack.files()[0];
        assert_eq!(file.unit_number, 17);
        assert_eq!(file.n_sections, 2);
        assert_eq!(file.file_action, "READ");
        assert!(file.eof);
    }

    #[test]
    fn testiofiles_program_flow_leaves_file1_on_stack() {
        let stack = run_testiofiles_scenario();
        assert_eq!(stack.files().len(), 1);
        assert_eq!(stack.files()[0].file_name, "file1");
        assert_eq!(stack.files()[0].unit_number, 1);
        assert_eq!(stack.files()[0].n_sections, 1);
        assert_eq!(stack.files()[0].file_action, "READWRITE");
        assert!(stack.files()[0].eof);
    }
}
