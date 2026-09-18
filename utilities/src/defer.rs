// Copyright (c) 2019-2026 Provable Inc.
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

/// A `Defer` holds a closure that is executed when the instance is dropped.
pub struct Defer<F: FnOnce()> {
    f: Option<F>,
}

impl<F: FnOnce()> Defer<F> {
    pub fn new(f: F) -> Self {
        Defer { f: Some(f) }
    }

    /// Prevents the deferred closure from running on drop.
    pub fn disarm(mut self) {
        self.f.take();
    }
}

impl<F: FnOnce()> Drop for Defer<F> {
    fn drop(&mut self) {
        if let Some(f) = self.f.take() {
            f();
        }
    }
}

#[macro_export]
macro_rules! defer {
    ($($body:tt)*) => {
        let _defer_guard = $crate::Defer::new(|| { $($body)* });
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn disarm_skips_the_deferred_closure() {
        let ran = AtomicBool::new(false);
        {
            let guard = Defer::new(|| ran.store(true, Ordering::SeqCst));
            guard.disarm();
        }
        assert!(!ran.load(Ordering::SeqCst));
    }
}
