//! The pictures GPUI reads by URL (button icons, games' art): a few threads that download them
//! with ureq, so a long list does not open hundreds of connections at once.

use std::{
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

use futures::{FutureExt as _, channel::oneshot, future::BoxFuture};
use http_client::{AsyncBody, HttpClient, Request, Response, Url, http::HeaderValue};

/// How many pictures download at once.
const WORKERS: usize = 4;
/// No picture is larger than this.
const LIMIT: u64 = 16 << 20;

type Job = (String, oneshot::Sender<anyhow::Result<(u16, Vec<u8>)>>);

pub struct Pictures {
    jobs: Mutex<mpsc::Sender<Job>>,
}

impl Default for Pictures {
    fn default() -> Self {
        let (jobs, queue) = mpsc::channel::<Job>();
        let queue = Arc::new(Mutex::new(queue));
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(30)))
            .user_agent("windows-deck")
            .build()
            .into();
        for index in 0..WORKERS {
            let (queue, agent) = (queue.clone(), agent.clone());
            let spawned = thread::Builder::new()
                .name(format!("pictures-{index}"))
                .spawn(move || {
                    loop {
                        let job = queue
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .recv();
                        let Ok((url, answer)) = job else { return };
                        let _ = answer.send(fetch(&agent, &url));
                    }
                });
            if spawned.is_err() {
                tracing::warn!("cannot start a thread for pictures");
            }
        }
        Self {
            jobs: Mutex::new(jobs),
        }
    }
}

fn fetch(agent: &ureq::Agent, url: &str) -> anyhow::Result<(u16, Vec<u8>)> {
    let mut response = agent.get(url).call()?;
    let status = response.status().as_u16();
    let bytes = response
        .body_mut()
        .with_config()
        .limit(LIMIT)
        .read_to_vec()?;
    Ok((status, bytes))
}

impl HttpClient for Pictures {
    fn user_agent(&self) -> Option<&HeaderValue> {
        None
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }

    fn send(
        &self,
        request: Request<AsyncBody>,
    ) -> BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
        let (answer, answered) = oneshot::channel();
        let sent = self
            .jobs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .send((request.uri().to_string(), answer));
        async move {
            sent.map_err(|_| anyhow::anyhow!("the picture threads stopped"))?;
            let (status, bytes) = answered.await??;
            Ok(Response::builder()
                .status(status)
                .body(AsyncBody::from(bytes))?)
        }
        .boxed()
    }
}
