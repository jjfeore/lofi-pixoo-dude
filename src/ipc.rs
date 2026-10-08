use crate::event::{Event, Kind};
#[cfg(not(windows))]
use anyhow::bail;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    future::Future,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    sync::{Semaphore, mpsc},
    time::{Duration, timeout},
};

const MAX_MESSAGE: u64 = 16 * 1024;

#[derive(Deserialize, Serialize)]
#[serde(
    tag = "op",
    content = "data",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum Request {
    Event(Event),
    Status,
}

fn retryable(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<tokio::time::error::Elapsed>()
        .is_some()
        || error
            .downcast_ref::<std::io::Error>()
            .is_some_and(|error| {
                matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::BrokenPipe
                        | std::io::ErrorKind::ConnectionReset
                )
            })
}

async fn deliver_event<F, Fut>(kind: Kind, mut attempt: F) -> Result<()>
where
    F: FnMut(u64) -> Fut,
    Fut: Future<Output = Result<()>>,
{
    // Completion has no later tool event to repair a lost observation. Allow
    // scheduling jitter with a combined IPC budget of at most 490 ms.
    // Other synchronous hooks retain their short existing IPC budget.
    let (attempts, budget_ms) = if kind == Kind::Complete {
        (3, 150)
    } else {
        (1, 50)
    };
    for index in 0..attempts {
        match attempt(budget_ms).await {
            Err(error) if index + 1 < attempts && retryable(&error) => {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            result => return result,
        }
    }
    unreachable!()
}

pub async fn send_event(pipe: &str, event: &Event) -> Result<()> {
    // Reuse the exact observation on retry. The engine's terminal fences make
    // replay safe if delivery succeeded but its acknowledgement was delayed.
    deliver_event(event.kind, |budget_ms| async move {
        send(pipe, Request::Event(event.clone()), budget_ms)
            .await
            .map(|_| ())
    })
    .await
}

#[cfg(windows)]
pub async fn serve(pipe: String, tx: mpsc::Sender<Event>, status: Arc<Mutex<Value>>) -> Result<()> {
    let permits = Arc::new(Semaphore::new(32));
    let descriptor = crate::winpipe::descriptor()?;
    let mut server = crate::winpipe::create(&pipe, true, &descriptor)
        .context("create pipe; another bridge may already be running")?;
    loop {
        server.connect().await?;
        let connected = server;
        // Always leave a listening instance available, including while one client is read.
        server = crate::winpipe::create(&pipe, false, &descriptor)?;
        let Ok(permit) = permits.clone().try_acquire_owned() else {
            continue;
        };
        let tx = tx.clone();
        let status = status.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let _ = timeout(Duration::from_millis(500), async move {
                let mut connected = connected;
                let mut bytes = Vec::new();
                BufReader::new(&mut connected)
                    .take(MAX_MESSAGE + 1)
                    .read_until(b'\n', &mut bytes)
                    .await?;
                if bytes.len() as u64 > MAX_MESSAGE {
                    return Ok::<_, anyhow::Error>(());
                }
                match serde_json::from_slice::<Request>(&bytes)? {
                    Request::Event(event) => {
                        event.validate()?;
                        // The state loop consumes immediately; it never waits on HTTP.
                        tx.send(event).await?;
                        // Confirm queue acceptance before a short-lived emitter closes
                        // its handle; pipe Flush alone isn't a delivery acknowledgment.
                        connected.write_all(b"{}\n").await?;
                        connected.flush().await?;
                    }
                    Request::Status => {
                        let value = status.lock().unwrap().clone();
                        connected.write_all(&serde_json::to_vec(&value)?).await?;
                        connected.write_all(b"\n").await?;
                        connected.flush().await?;
                    }
                }
                Ok(())
            })
            .await;
        });
    }
}

#[cfg(not(windows))]
pub async fn serve(_: String, _: mpsc::Sender<Event>, _: Arc<Mutex<Value>>) -> Result<()> {
    bail!("this release supports Windows named pipes; replay/asset tools are portable")
}

#[cfg(windows)]
pub async fn send(pipe: &str, request: Request, budget_ms: u64) -> Result<Option<Value>> {
    use tokio::net::windows::named_pipe::ClientOptions;
    timeout(Duration::from_millis(budget_ms), async {
        let status_request = matches!(request, Request::Status);
        let mut client = loop {
            match ClientOptions::new().open(pipe) {
                Ok(client) => break client,
                Err(error) if error.raw_os_error() == Some(231) => {
                    tokio::time::sleep(Duration::from_millis(2)).await
                }
                Err(error) => return Err(error.into()),
            }
        };
        client.write_all(&serde_json::to_vec(&request)?).await?;
        // Pipe messages use a newline terminator, not disconnect, for duplex status.
        client.write_all(b"\n").await?;
        client.flush().await?;
        let mut data = Vec::new();
        // The reply is a framed acknowledgment. Do not wait for handle closure
        // after receiving it; Windows disconnect scheduling is unrelated to
        // queue acceptance and should not consume the emitter's IPC budget.
        BufReader::new(&mut client)
            .take(MAX_MESSAGE + 1)
            .read_until(b'\n', &mut data)
            .await?;
        anyhow::ensure!(
            data.len() as u64 <= MAX_MESSAGE && data.last() == Some(&b'\n'),
            "invalid or incomplete IPC reply"
        );
        let value = serde_json::from_slice(&data)?;
        Ok(if status_request { Some(value) } else { None })
    })
    .await
    .context("IPC deadline exceeded")?
}

#[cfg(not(windows))]
pub async fn send(_: &str, _: Request, _: u64) -> Result<Option<Value>> {
    bail!("named-pipe forwarding requires Windows")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error, ErrorKind};

    #[tokio::test]
    async fn completion_retries_a_transient_failure_and_stops_on_success() {
        let mut budgets = Vec::new();
        deliver_event(Kind::Complete, |budget| {
            budgets.push(budget);
            std::future::ready(if budgets.len() == 1 {
                Err(Error::from(ErrorKind::TimedOut).into())
            } else { Ok(()) })
        }).await.unwrap();
        assert_eq!(budgets, [150, 150]);
    }

    #[tokio::test]
    async fn completion_retries_contextual_tokio_deadline_errors() {
        let elapsed = timeout(Duration::ZERO, std::future::pending::<()>()).await.unwrap_err();
        let mut failure = Some(anyhow::Error::new(elapsed).context("IPC deadline exceeded"));
        let mut attempts = 0;
        deliver_event(Kind::Complete, |_| {
            attempts += 1;
            std::future::ready(failure.take().map_or(Ok(()), Err))
        }).await.unwrap();
        assert_eq!(attempts, 2);
    }

    #[tokio::test]
    async fn completion_retries_are_bounded_and_preserve_the_error() {
        let mut attempts = 0;
        let error = deliver_event(Kind::Complete, |_| {
            attempts += 1;
            std::future::ready(Err(Error::from(ErrorKind::BrokenPipe).into()))
        }).await.unwrap_err();
        assert_eq!(attempts, 3);
        assert_eq!(error.downcast_ref::<Error>().unwrap().kind(), ErrorKind::BrokenPipe);
    }

    #[tokio::test]
    async fn permission_failures_and_synchronous_hooks_do_not_retry() {
        for (kind, failure, budget) in [(Kind::Complete, ErrorKind::PermissionDenied, 150),
            (Kind::Prompt, ErrorKind::TimedOut, 50)] {
            let mut attempts = 0;
            let error = deliver_event(kind, |actual_budget| {
                attempts += 1;
                assert_eq!(actual_budget, budget);
                std::future::ready(Err(Error::from(failure).into()))
            }).await.unwrap_err();
            assert_eq!(attempts, 1);
            assert_eq!(error.downcast_ref::<Error>().unwrap().kind(), failure);
        }
    }
}
