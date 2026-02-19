use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct ErrorStack {
    messages: Vec<String>,
    print_error_stack: bool,
    num_spaces: usize,
}

impl ErrorStack {
    pub fn new(printing_is_on: bool) -> Self {
        let mut stack = Self::default();
        stack.init_error_stack(Some(printing_is_on));
        stack
    }

    pub fn init_error_stack(&mut self, printing_is_on: Option<bool>) {
        self.messages.clear();
        self.num_spaces = 0;
        self.print_error_stack = printing_is_on.unwrap_or(false);
    }

    pub fn next_error(&mut self, message: impl Into<String>) {
        self.messages.push(message.into());
        if !self.print_error_stack {
            return;
        }

        if let Some(last) = self.messages.last() {
            println!("{}-->{}", " ".repeat(self.num_spaces), last.trim());
            println!("{}|", " ".repeat(self.num_spaces + 2));
            self.num_spaces += 2;
        }
    }

    pub fn delete_error(&mut self) -> Option<String> {
        let removed = self.messages.pop()?;
        if self.print_error_stack {
            let line_indent = self.num_spaces.saturating_sub(2);
            let bar_indent = self.num_spaces.saturating_sub(4);
            println!("{}<--{}", " ".repeat(line_indent), removed.trim());
            println!("{}|", " ".repeat(bar_indent));
            self.num_spaces = self.num_spaces.saturating_sub(2);
        }
        Some(removed)
    }

    pub fn dump_error_stack<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let mut file = File::create(path)?;
        let mut indent = 0usize;

        for message in &self.messages {
            let trimmed = message.trim();
            let header = format!("{}--> {}", " ".repeat(indent), trimmed);
            let bar = format!("{}|", " ".repeat(indent + trimmed.len() + 2));

            println!("{header}");
            println!("{bar}");
            writeln!(file, "{header}")?;
            writeln!(file, "{bar}")?;

            indent += trimmed.len() + 2;
        }

        Ok(())
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn messages(&self) -> &[String] {
        &self.messages
    }
}

#[cfg(test)]
mod tests {
    use super::ErrorStack;

    #[test]
    fn stack_push_and_pop_behave_like_lifo() {
        let mut stack = ErrorStack::new(false);
        stack.next_error("first");
        stack.next_error("second");

        assert_eq!(stack.len(), 2);
        assert_eq!(stack.delete_error().as_deref(), Some("second"));
        assert_eq!(stack.delete_error().as_deref(), Some("first"));
        assert_eq!(stack.delete_error(), None);
    }

    #[test]
    fn dump_error_stack_writes_expected_markers() {
        let temp = tempfile::NamedTempFile::new().expect("temp file");
        let path = temp.path().to_path_buf();

        let mut stack = ErrorStack::new(false);
        stack.next_error("outer");
        stack.next_error("inner");
        stack.dump_error_stack(&path).expect("dump should succeed");

        let content = std::fs::read_to_string(path).expect("read dump");
        assert!(content.contains("--> outer"));
        assert!(content.contains("--> inner"));
        assert!(content.contains('|'));
    }
}
