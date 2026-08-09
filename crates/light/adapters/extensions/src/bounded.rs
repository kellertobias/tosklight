use std::collections::VecDeque;

/// A byte-capped newest-tail log. Old lines are discarded before new diagnostics so an extension
/// cannot grow host memory without limit, while the most actionable recent output remains.
#[derive(Debug)]
pub(crate) struct BoundedLog {
    lines: VecDeque<String>,
    bytes: usize,
    max_bytes: usize,
    max_lines: usize,
}

impl BoundedLog {
    pub(crate) fn new(max_bytes: usize, max_lines: usize) -> Self {
        Self {
            lines: VecDeque::new(),
            bytes: 0,
            max_bytes,
            max_lines,
        }
    }

    /// Returns the number of lines evicted or refused.
    pub(crate) fn push(&mut self, mut line: String) -> u64 {
        let mut dropped = 0;
        if self.max_bytes == 0 || self.max_lines == 0 {
            return 1;
        }
        if line.len() > self.max_bytes {
            while line.len() > self.max_bytes {
                let first = line.chars().next().expect("non-empty oversized log line");
                line.drain(..first.len_utf8());
            }
            dropped += 1;
        }
        while self.lines.len() >= self.max_lines || self.bytes + line.len() > self.max_bytes {
            let Some(removed) = self.lines.pop_front() else {
                break;
            };
            self.bytes -= removed.len();
            dropped += 1;
        }
        self.bytes += line.len();
        self.lines.push_back(line);
        dropped
    }

    pub(crate) fn snapshot(&self) -> (Vec<String>, usize) {
        (self.lines.iter().cloned().collect(), self.bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_retains_only_the_bounded_newest_tail() {
        let mut log = BoundedLog::new(12, 2);
        assert_eq!(log.push("first".into()), 0);
        assert_eq!(log.push("second".into()), 0);
        assert_eq!(log.push("third".into()), 1);
        let (lines, bytes) = log.snapshot();
        assert_eq!(lines, vec!["second", "third"]);
        assert!(bytes <= 12);
    }
}
