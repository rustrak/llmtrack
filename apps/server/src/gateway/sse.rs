//! Server-sent events, split at event boundaries.
//!
//! Network chunks cut events anywhere, so bytes are buffered until a blank
//! line closes an event. Each event comes out whole, with its raw bytes (to
//! forward untouched) and its `data:` payload (to inspect).

pub struct Event {
    /// The event exactly as received, including the closing blank line.
    pub raw: Vec<u8>,
    /// The `data:` lines joined by `\n`, or empty when there were none.
    pub data: String,
}

#[derive(Default)]
pub struct SseParser {
    buffer: Vec<u8>,
}

impl SseParser {
    /// Feeds bytes in and returns every event they completed.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<Event> {
        self.buffer.extend_from_slice(bytes);
        let mut events = Vec::new();
        while let Some(end) = event_end(&self.buffer) {
            let raw: Vec<u8> = self.buffer.drain(..end).collect();
            events.push(Event {
                data: data_of(&raw),
                raw,
            });
        }
        events
    }

    /// Whatever was left without a closing blank line when the stream ended.
    pub fn finish(&mut self) -> Option<Event> {
        if self.buffer.iter().all(u8::is_ascii_whitespace) {
            return None;
        }
        let raw = std::mem::take(&mut self.buffer);
        Some(Event {
            data: data_of(&raw),
            raw,
        })
    }
}

/// The index just past the first blank line (`\n\n` or `\r\n\r\n`).
fn event_end(buffer: &[u8]) -> Option<usize> {
    let lf = buffer.windows(2).position(|w| w == b"\n\n").map(|i| i + 2);
    let crlf = buffer
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| i + 4);
    match (lf, crlf) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

fn data_of(raw: &[u8]) -> String {
    String::from_utf8_lossy(raw)
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(|data| data.strip_prefix(' ').unwrap_or(data))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_split_across_chunks_come_out_whole() {
        let mut parser = SseParser::default();
        assert!(parser.push(b"data: {\"a\"").is_empty());
        let events = parser.push(b":1}\n\ndata: [DONE]\n\n");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].data, "{\"a\":1}");
        assert_eq!(events[0].raw, b"data: {\"a\":1}\n\n");
        assert_eq!(events[1].data, "[DONE]");
    }

    #[test]
    fn event_lines_and_crlf_are_handled() {
        let mut parser = SseParser::default();
        let events = parser.push(b"event: ping\r\ndata: {}\r\n\r\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "{}");
    }

    #[test]
    fn a_trailing_event_without_blank_line_is_flushed_at_the_end() {
        let mut parser = SseParser::default();
        assert!(parser.push(b"data: last").is_empty());
        assert_eq!(parser.finish().unwrap().data, "last");
        assert!(parser.finish().is_none());
    }

    #[test]
    fn comments_have_no_data() {
        let mut parser = SseParser::default();
        assert_eq!(parser.push(b": keep-alive\n\n")[0].data, "");
    }
}
