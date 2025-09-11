// Copyright (c) 2019-2025 Provable Inc.
// This file is part of the snarkVM library.

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:

// http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

/// Wrapper around `tokio::JoinHandle` that propagates panics.
pub struct JoinHandle<R: Send + 'static> {
    inner: tokio::task::JoinHandle<R>,
}

/// Wrapper around `tokio::spawn_blocking` that propagates panics.
pub fn spawn_blocking<F, R>(f: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    JoinHandle { inner: tokio::task::spawn_blocking(f) }
}

/// Wrapper around `tokio::spawn` that propagates panics.
pub fn spawn<F>(f: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    JoinHandle { inner: tokio::task::spawn(f) }
}

impl<R: Send + 'static> JoinHandle<R> {
    pub fn abort(&self) {
        self.inner.abort();
    }
}

impl<R: Send + 'static> Future for JoinHandle<R> {
    type Output = R;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Poll::Ready(result) = std::pin::pin!(&mut self.inner).poll(cx) else {
            return Poll::Pending;
        };

        match result {
            Ok(value) => Poll::Ready(value),
            Err(err) => {
                if err.is_panic() {
                    // Resume the panic on the main task
                    std::panic::resume_unwind(err.into_panic());
                } else {
                    panic!("Got unexpected tokio error: {err}");
                }
            }
        }
    }
}
