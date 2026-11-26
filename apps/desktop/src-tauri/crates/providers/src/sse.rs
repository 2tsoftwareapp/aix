//! Server-sent events parser — incremental, feed bytes, drain events.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
}

/// Incremental SSE decoder. Feed arbitrary byte chunks; call `drain` for
/// completed events. Handles multi-line data fields and CRLF.
#[derive(Default)]
pub struct SseDecoder {
    buf: String,
    cur_event: Option<String>,
    cur_data: Vec<String>,
}

impl SseDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn feed(&mut self, chunk: &str) -> Vec<SseEvent> {
        self.buf.push_str(&chunk.replace("\r\n", "\n"));
        let mut out = Vec::new();
        while let Some(pos) = self.buf.find('\n') {
            let line: String = self.buf.drain(..=pos).collect();
            let line = line.trim_end_matches('\n');
            if line.is_empty() {
                // blank line terminates an event
                if !self.cur_data.is_empty() {
                    out.push(SseEvent {
                        event: self.cur_event.take(),
                        data: self.cur_data.join("\n"),
                    });
                    self.cur_data.clear();
                }
                continue;
            }
            if line.starts_with(':') {
                continue; // comment/heartbeat
            }
            if let Some(v) = line.strip_prefix("event:") {
                self.cur_event = Some(v.trim().to_string());
            } else if let Some(v) = line.strip_prefix("data:") {
                self.cur_data.push(v.strip_prefix(' ').unwrap_or(v).to_string());
            }
            // id:/retry: ignored — not needed for LLM streams
        }
        out
    }

    /// Flush at end-of-stream: parse any unterminated trailing line, then emit
    /// any pending event.
    pub fn finish(&mut self) -> Vec<SseEvent> {
        let mut out = Vec::new();
        if !self.buf.is_empty() {
            let remainder = std::mem::take(&mut self.buf);
            // re-parse the unterminated trailing line as complete;
            // the trailing newline terminates the line but NOT the event,
            // so the cur_data flush below still applies
            out.extend(self.feed(&format!("{remainder}\n")));
        }
        if !self.cur_data.is_empty() {
            out.push(SseEvent {
                event: self.cur_event.take(),
                data: self.cur_data.join("\n"),
            });
            self.cur_data.clear();
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_event() {
        let mut d = SseDecoder::new();
        let evs = d.feed("event: message_start\ndata: {\"a\":1}\n\n");
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].event.as_deref(), Some("message_start"));
        assert_eq!(evs[0].data, "{\"a\":1}");
    }

    #[test]
    fn handles_split_chunks() {
        let mut d = SseDecoder::new();
        assert!(d.feed("data: {\"a\"").is_empty());
        let evs = d.feed(":1}\n\ndata: second\n\n");
        assert_eq!(evs.len(), 2);
        assert_eq!(evs[0].data, "{\"a\":1}");
        assert_eq!(evs[1].data, "second");
    }

    #[test]
    fn multiline_data_joined() {
        let mut d = SseDecoder::new();
        let evs = d.feed("data: line1\ndata: line2\n\n");
        assert_eq!(evs[0].data, "line1\nline2");
    }

    #[test]
    fn comments_ignored() {
        let mut d = SseDecoder::new();
        let evs = d.feed(": heartbeat\n\ndata: real\n\n");
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].data, "real");
    }

    #[test]
    fn finish_flushes_tail() {
        let mut d = SseDecoder::new();
        d.feed("data: tail");
        let evs = d.finish();
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].data, "tail");
    }
}
