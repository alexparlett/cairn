//! The settings every diff view shares (PRD R6.1, R6.3, R6.7): how much context, the entire
//! file or not, whitespace ignored or not. One value for the session, held by the
//! application; not remembered across sessions (issue #29).

use cairn_model::Context;

/// The diff views' shared settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffSettings {
    /// Lines of context around each change, never below one. Kept while the entire file is
    /// shown, so leaving that mode returns to it.
    lines: u32,
    entire_file: bool,
    ignore_whitespace: bool,
    /// Whether the user has moved the context this session: until they have, the context
    /// follows the configuration's `diff.context` once the engine has read it.
    chosen: bool,
}

impl Default for DiffSettings {
    /// git's default context, until the configuration's is known.
    fn default() -> Self {
        Self {
            lines: Context::DEFAULT_LINES,
            entire_file: false,
            ignore_whitespace: false,
            chosen: false,
        }
    }
}

impl DiffSettings {
    /// The context a view asks at and groups at.
    pub fn context(self) -> Context {
        if self.entire_file {
            Context::EntireFile
        } else {
            Context::lines(self.lines)
        }
    }

    pub fn lines(self) -> u32 {
        self.lines
    }

    pub fn entire_file(self) -> bool {
        self.entire_file
    }

    pub fn ignore_whitespace(self) -> bool {
        self.ignore_whitespace
    }

    /// The user's `git diff` context, read from their configuration: taken as the session's
    /// starting point unless the context was already moved. Returns whether it changed
    /// anything.
    pub fn configured(&mut self, context: Context) -> bool {
        let lines = context.line_count().unwrap_or(Context::DEFAULT_LINES);
        if self.chosen || self.lines == lines {
            return false;
        }
        self.lines = lines;
        true
    }

    /// One more line of context (R6.3). Does nothing while the entire file is shown.
    pub fn more_lines(&mut self) -> bool {
        if self.entire_file {
            return false;
        }
        self.lines = self.lines.saturating_add(1);
        self.chosen = true;
        true
    }

    /// One line fewer, never below one (R6.3). Does nothing while the entire file is shown.
    pub fn fewer_lines(&mut self) -> bool {
        if self.entire_file || self.lines <= 1 {
            return false;
        }
        self.lines -= 1;
        self.chosen = true;
        true
    }

    pub fn can_show_fewer_lines(self) -> bool {
        !self.entire_file && self.lines > 1
    }

    pub fn can_show_more_lines(self) -> bool {
        !self.entire_file
    }

    /// The entire file, or back to the lines of context it left.
    pub fn toggle_entire_file(&mut self) {
        self.entire_file = !self.entire_file;
    }

    pub fn toggle_ignore_whitespace(&mut self) {
        self.ignore_whitespace = !self.ignore_whitespace;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R6.3: a line per click, never below one; the entire file shows every line and gives
    /// back the count it left. Caught by: a floor of zero, a step of more than one, or the
    /// entire file forgetting the lines.
    #[test]
    fn context_moves_a_line_at_a_time_and_never_below_one() {
        let mut settings = DiffSettings::default();
        assert_eq!(settings.context(), Context::Lines(3));
        assert!(settings.fewer_lines());
        assert!(settings.fewer_lines());
        assert_eq!(settings.context(), Context::Lines(1));
        assert!(!settings.can_show_fewer_lines());
        assert!(!settings.fewer_lines(), "the context went below one line");
        assert_eq!(settings.context(), Context::Lines(1));
        assert!(settings.more_lines());
        assert_eq!(settings.context(), Context::Lines(2));

        settings.toggle_entire_file();
        assert_eq!(settings.context(), Context::EntireFile);
        assert!(!settings.more_lines() && !settings.fewer_lines());
        assert!(!settings.can_show_more_lines() && !settings.can_show_fewer_lines());
        settings.toggle_entire_file();
        assert_eq!(settings.context(), Context::Lines(2));
    }

    /// The configured context is where the session starts, until the user moves it; zero is
    /// raised to one. Caught by: the configuration overriding a context the user chose.
    #[test]
    fn the_configured_context_is_taken_until_the_user_moves_it() {
        let mut settings = DiffSettings::default();
        assert!(settings.configured(Context::Lines(5)));
        assert_eq!(settings.context(), Context::Lines(5));
        assert!(!settings.configured(Context::Lines(5)), "nothing changed");
        assert!(settings.configured(Context::lines(0)));
        assert_eq!(settings.context(), Context::Lines(1));

        assert!(settings.more_lines());
        assert!(!settings.configured(Context::Lines(9)));
        assert_eq!(settings.context(), Context::Lines(2));
    }

    #[test]
    fn whitespace_toggles() {
        let mut settings = DiffSettings::default();
        assert!(!settings.ignore_whitespace());
        settings.toggle_ignore_whitespace();
        assert!(settings.ignore_whitespace());
        settings.toggle_ignore_whitespace();
        assert!(!settings.ignore_whitespace());
    }
}
