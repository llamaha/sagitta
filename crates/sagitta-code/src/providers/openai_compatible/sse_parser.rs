use eventsource_stream::EventStream;
use futures::{Stream, StreamExt};
use log::{debug, trace, warn};
use pin_project::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};

#[derive(Debug, Clone)]
pub enum SseEvent {
    Message(serde_json::Value),
    Done,
}

#[pin_project]
pub struct SseParser<S> {
    #[pin]
    event_stream: EventStream<S>,
}

impl<S> SseParser<S>
where
    S: Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + Unpin,
{
    pub fn new(stream: S) -> Self {
        Self {
            event_stream: EventStream::new(stream),
        }
    }
}

impl<S> Stream for SseParser<S>
where
    S: Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + Unpin,
{
    type Item = Result<SseEvent, Box<dyn std::error::Error + Send + Sync>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        loop {
            match this.event_stream.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(event))) => {
                    trace!("SSE event: {:?}", event);

                    // Check if this is the [DONE] marker
                    if event.data.trim() == "[DONE]" {
                        return Poll::Ready(Some(Ok(SseEvent::Done)));
                    }

                    // Try to parse as JSON
                    match serde_json::from_str::<serde_json::Value>(&event.data) {
                        Ok(json) => {
                            return Poll::Ready(Some(Ok(SseEvent::Message(json))));
                        }
                        Err(e) => {
                            warn!("Failed to parse SSE JSON: {}, data: {}", e, event.data);
                            // Skip invalid JSON
                            continue;
                        }
                    }
                }
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)));
                }
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;

    fn create_test_sse_line(content: &str) -> bytes::Bytes {
        bytes::Bytes::from(format!("data: {}\n\n", content))
    }

    #[tokio::test]
    async fn test_parse_message() {
        let data = r#"{"choices":[{"delta":{"content":"Hello"}}]}"#;
        let stream = stream::iter(vec![Ok(create_test_sse_line(data))]);
        
        let mut parser = SseParser::new(stream);
        let event = parser.next().await.unwrap().unwrap();
        
        match event {
            SseEvent::Message(json) => {
                assert_eq!(json["choices"][0]["delta"]["content"], "Hello");
            }
            _ => panic!("Expected message event"),
        }
    }

    #[tokio::test]
    async fn test_parse_done() {
        let stream = stream::iter(vec![Ok(create_test_sse_line("[DONE]"))]);
        
        let mut parser = SseParser::new(stream);
        let event = parser.next().await.unwrap().unwrap();
        
        match event {
            SseEvent::Done => {} // Expected
            _ => panic!("Expected done event"),
        }
    }

    #[tokio::test]
    async fn test_skip_invalid_json() {
        let stream = stream::iter(vec![
            Ok(create_test_sse_line("invalid json")),
            Ok(create_test_sse_line(r#"{"valid":"json"}"#)),
        ]);
        
        let mut parser = SseParser::new(stream);
        let event = parser.next().await.unwrap().unwrap();
        
        match event {
            SseEvent::Message(json) => {
                assert_eq!(json["valid"], "json");
            }
            _ => panic!("Expected message event"),
        }
    }
}