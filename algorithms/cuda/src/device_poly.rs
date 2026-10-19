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

//! Polynomial coefficients kept on the device after the call that computed
//! them, for later calls to read in place of uploading the host copy.
//!
//! A [`DevicePoly`] owns its memory through an `Arc`, from the resident pool on
//! device 0, and the last clone to drop frees it. A [`DeviceCopy`] pairs one
//! with the host coefficients it mirrors, and a consumer reads the device copy
//! only for those same host coefficients. Anything else, including more than
//! one GPU, takes the host path.

use core::ffi::c_void;
use std::{
    cell::RefCell,
    fmt,
    io::{Read, Write},
    marker::PhantomData,
    sync::{
        Arc,
        LazyLock,
        atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed},
    },
};

use snarkvm_utilities::{CanonicalDeserialize, CanonicalSerialize, Compress, SerializationError, Valid, Validate};

use super::{FR_BYTES, Fr, assert_type};

// SAFETY: no arguments.
static SUPPORTED: LazyLock<bool> = LazyLock::new(|| unsafe { super::snarkvm_bases_status() }.code == 0);

static RETAINED: AtomicU64 = AtomicU64::new(0);
static RETAIN_FAILURES: AtomicU64 = AtomicU64::new(0);
static DEVICE_READS: AtomicU64 = AtomicU64::new(0);
static HOST_FALLBACKS: AtomicU64 = AtomicU64::new(0);
static HELD_BYTES: AtomicUsize = AtomicUsize::new(0);

/// Whether a call should ask the device to keep its outputs.
pub(crate) fn supported() -> bool {
    *SUPPORTED
}

struct Buffer {
    ptr: usize,
    bytes: usize,
}

impl Drop for Buffer {
    fn drop(&mut self) {
        // SAFETY: `ptr` came from the resident pool and is freed once. Every
        // call that read it has returned with its device work complete.
        unsafe { super::snarkvm_bases_release(self.ptr as *mut c_void) };
        HELD_BYTES.fetch_sub(self.bytes, Relaxed);
    }
}

/// `len` field elements on device 0, from `offset` elements into a buffer.
#[derive(Clone)]
pub struct DevicePoly {
    buf: Arc<Buffer>,
    offset: usize,
    len: usize,
}

impl fmt::Debug for DevicePoly {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DevicePoly({})", self.len)
    }
}

impl DevicePoly {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The device ordinal the coefficients are on.
    pub fn device(&self) -> usize {
        0
    }

    fn ptr(&self) -> *const c_void {
        (self.buf.ptr + self.offset * FR_BYTES) as *const c_void
    }

    pub fn download<T: 'static>(&self, out: &mut [T]) -> Result<(), super::cuda::Error> {
        assert_type::<T, Fr>();
        assert_eq!(out.len(), self.len, "the output is the device copy's length");
        // SAFETY: `out` holds `len` elements and the buffer holds `len` from `ptr()`.
        let err = unsafe {
            super::snarkvm_resident_download(out.as_mut_ptr() as *mut c_void, self.ptr(), self.len * FR_BYTES)
        };
        if err.code != 0 { Err(err) } else { Ok(()) }
    }
}

/// Takes ownership of what `FourthRoundChain` kept, `domain` elements each, as
/// `a`, `b` and `g = f[1..]`. `None` if the device kept nothing.
pub(crate) fn adopt(keep: [*mut c_void; 3], domain: usize) -> Option<[DevicePoly; 3]> {
    let bytes = domain * FR_BYTES;
    let bufs = keep.map(|ptr| {
        (!ptr.is_null()).then(|| {
            HELD_BYTES.fetch_add(bytes, Relaxed);
            Arc::new(Buffer { ptr: ptr as usize, bytes })
        })
    });
    let [Some(a), Some(b), Some(f)] = bufs else {
        RETAIN_FAILURES.fetch_add(1, Relaxed);
        return None;
    };
    RETAINED.fetch_add(1, Relaxed);
    Some([DevicePoly { buf: a, offset: 0, len: domain }, DevicePoly { buf: b, offset: 0, len: domain }, DevicePoly {
        buf: f,
        offset: 1,
        len: domain - 1,
    }])
}

/// A [`DevicePoly`] and the host coefficients it mirrors, or nothing.
///
/// Serializes as nothing and compares equal to any other, so a polynomial's
/// bytes and equality do not depend on it.
#[derive(Clone, Default)]
pub struct DeviceCopy(Option<(DevicePoly, usize, usize)>);

impl DeviceCopy {
    /// `device` as the copy of `host`, which must be a prefix of it.
    pub fn new<T: 'static>(device: DevicePoly, host: &[T]) -> Self {
        assert_type::<T, Fr>();
        assert!(host.len() <= device.len, "{} host coefficients for a device copy of {}", host.len(), device.len);
        Self(Some((device, host.as_ptr() as usize, host.len())))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_none()
    }

    /// The device copy if this mirrors `host`, counting the read or, for a copy
    /// of other coefficients, the fallback.
    fn get<T>(&self, host: &[T]) -> Option<&DevicePoly> {
        let (device, ptr, len) = self.0.as_ref()?;
        if host.as_ptr() as usize == *ptr && host.len() == *len {
            Some(device)
        } else {
            HOST_FALLBACKS.fetch_add(1, Relaxed);
            None
        }
    }

    /// The device pointer an opening term reads, or null for the host path.
    pub(crate) fn term<T>(&self, host: &[T]) -> *const c_void {
        match self.get(host) {
            Some(device) if !host.is_empty() => {
                DEVICE_READS.fetch_add(1, Relaxed);
                device.ptr()
            }
            _ => std::ptr::null(),
        }
    }
}

impl fmt::Debug for DeviceCopy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Some((device, ..)) => device.fmt(f),
            None => f.write_str("None"),
        }
    }
}

impl PartialEq for DeviceCopy {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for DeviceCopy {}

impl CanonicalSerialize for DeviceCopy {
    fn serialize_with_mode<W: Write>(&self, _: W, _: Compress) -> Result<(), SerializationError> {
        Ok(())
    }

    fn serialized_size(&self, _: Compress) -> usize {
        0
    }
}

impl Valid for DeviceCopy {
    fn check(&self) -> Result<(), SerializationError> {
        Ok(())
    }
}

impl CanonicalDeserialize for DeviceCopy {
    fn deserialize_with_mode<R: Read>(_: R, _: Compress, _: Validate) -> Result<Self, SerializationError> {
        Ok(Self::default())
    }
}

thread_local! {
    static LENT: RefCell<Vec<(DevicePoly, usize, usize)>> = const { RefCell::new(Vec::new()) };
}

/// Lends a device copy to the MSMs on this thread until dropped.
pub struct Lend<'a>(bool, PhantomData<&'a ()>);

impl Drop for Lend<'_> {
    fn drop(&mut self) {
        if self.0 {
            LENT.with_borrow_mut(|lent| lent.pop());
        }
    }
}

/// Until the guard drops, an MSM on this thread whose scalars lie in `host`
/// reads them from `copy`'s device copy, if `copy` mirrors `host`.
pub fn lend<'a, T>(copy: &DeviceCopy, host: &'a [T]) -> Lend<'a> {
    let Some(device) = copy.get(host) else { return Lend(false, PhantomData) };
    let entry = (device.clone(), host.as_ptr() as usize, host.len());
    LENT.with_borrow_mut(|lent| lent.push(entry));
    Lend(true, PhantomData)
}

/// The device copy of `scalars` if a [`lend`] on this thread covers them.
pub(crate) fn lent<T>(scalars: &[T]) -> *const c_void {
    let (start, size) = (scalars.as_ptr() as usize, size_of::<T>());
    LENT.with_borrow(|lent| {
        lent.iter().rev().find_map(|(device, host, len)| {
            let inside = size == FR_BYTES
                && start >= *host
                && (start - host).is_multiple_of(size)
                && (start - host) / size + scalars.len() <= *len;
            inside.then(|| {
                DEVICE_READS.fetch_add(1, Relaxed);
                (device.ptr() as usize + (start - host)) as *const c_void
            })
        })
    })
    .unwrap_or(std::ptr::null())
}

/// Counters since the process started.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Calls whose outputs the device kept.
    pub retained: u64,
    /// Calls that asked the device to keep their outputs and it could not.
    pub retain_failures: u64,
    /// MSMs and opening terms that read a device copy.
    pub device_reads: u64,
    /// Reads that had a device copy of other coefficients and uploaded.
    pub host_fallbacks: u64,
    /// Device bytes held.
    pub held_bytes: usize,
}

pub fn stats() -> Stats {
    Stats {
        retained: RETAINED.load(Relaxed),
        retain_failures: RETAIN_FAILURES.load(Relaxed),
        device_reads: DEVICE_READS.load(Relaxed),
        host_fallbacks: HOST_FALLBACKS.load(Relaxed),
        held_bytes: HELD_BYTES.load(Relaxed),
    }
}

/// Each consumer against the same call with no device copy, and again after the
/// host coefficients change, which moves the host path and not the device's.
/// Retention needs exactly one GPU, so each test returns early on any other
/// count. The tests hold `SERIAL`, so `held_bytes` moves only with them.
#[cfg(test)]
mod tests {
    use super::{DeviceCopy, DevicePoly, lend, stats};
    use crate::{G1Affine, G1Projective, accumulate_open, fourth_round_chain, msm};
    use snarkvm_curves::{ProjectiveCurve, bls12_377::Fr};
    use snarkvm_utilities::rand::{TestRng, Uniform};
    use std::sync::Mutex;

    static SERIAL: Mutex<()> = Mutex::new(());

    const LG: u32 = 10;

    fn one_gpu() -> bool {
        crate::device_count() == 1
    }

    struct Chain {
        a: Vec<Fr>,
        b: Vec<Fr>,
        g: Vec<Fr>,
        kept: Option<[DevicePoly; 3]>,
    }

    /// The chain over random evaluations of `len`, with arguments the divide
    /// accepts.
    fn chain(rng: &mut TestRng, lg: u32, len: usize) -> Chain {
        let mut array = || (0..len).map(|_| Fr::rand(rng)).collect::<Vec<_>>();
        let (row, col, rcv) = (array(), array(), array());
        let (alpha, beta, coeff, b_scale, selector_mult) =
            (Fr::rand(rng), Fr::rand(rng), Fr::rand(rng), Fr::rand(rng), Fr::rand(rng));
        let n = 1usize << lg;
        let (mut a, mut b, mut g, mut lhs, mut f0) = (
            vec![Fr::default(); n],
            vec![Fr::default(); n],
            vec![Fr::default(); n - 1],
            vec![Fr::default(); n - 1],
            Fr::default(),
        );
        let kept = fourth_round_chain(
            &mut a,
            &mut b,
            &mut g,
            &mut lhs,
            &mut f0,
            &row,
            &col,
            &rcv,
            lg,
            lg + 1,
            &alpha,
            &beta,
            &coeff,
            &b_scale,
            &(b_scale * coeff),
            &selector_mult,
        )
        .map_err(|e| e.code)
        .expect("the chain runs");
        Chain { a, b, g, kept }
    }

    fn bases(rng: &mut TestRng, n: usize) -> Vec<G1Affine> {
        G1Projective::batch_normalization_into_affine((0..n).map(|_| G1Projective::rand(rng)).collect())
    }

    fn download(d: &DevicePoly) -> Vec<Fr> {
        let mut out = vec![Fr::default(); d.len()];
        d.download(&mut out).map_err(|e| e.code).expect("the download runs");
        out
    }

    #[test]
    fn the_kept_outputs_are_the_downloaded_ones() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        if !one_gpu() {
            return;
        }
        let rng = &mut TestRng::default();
        for (lg, len) in [(LG, 1usize << LG), (12, (1 << 12) - 1025)] {
            let before = stats();
            let c = chain(rng, lg, len);
            let [a, b, g] = c.kept.expect("one GPU keeps the outputs");
            assert_eq!(stats().held_bytes, before.held_bytes + 3 * (32 << lg), "lg {lg}");
            assert_eq!(stats().retained, before.retained + 1, "lg {lg}");
            assert!(download(&a) == c.a, "lg {lg}: a");
            assert!(download(&b) == c.b, "lg {lg}: b");
            assert!(download(&g) == c.g, "lg {lg}: g");
            drop((a, b));
            assert_eq!(stats().held_bytes, before.held_bytes + (32 << lg), "lg {lg}: a and b are freed");
            drop(g);
            assert_eq!(stats().held_bytes, before.held_bytes, "lg {lg}: g is freed");
        }
    }

    /// `g` from a leading offset, as `skip_leading_zeros` slices it.
    #[test]
    fn a_lent_msm_reads_the_device_copy() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        if !one_gpu() {
            return;
        }
        let rng = &mut TestRng::default();
        let mut c = chain(rng, LG, 1 << LG);
        let [.., d_g] = c.kept.take().expect("one GPU keeps the outputs");
        let bases = bases(rng, c.g.len());
        let delta = Fr::rand(rng);
        let run = |g: &[Fr], copy: &DeviceCopy, k: usize| {
            let _lend = lend(copy, g);
            msm::<G1Affine, G1Projective, Fr>(&bases[k..], &g[k..])
                .map_err(|e| e.code)
                .expect("the MSM runs")
                .to_affine()
        };
        let none = DeviceCopy::default();
        for k in [0, 5] {
            let want = run(&c.g, &none, k);
            let copy = DeviceCopy::new(d_g.clone(), &c.g);
            let reads = stats().device_reads;
            assert_eq!(run(&c.g, &copy, k), want, "k {k}");
            assert_eq!(stats().device_reads, reads + 1, "k {k}: the MSM read the device copy");
            c.g[k] += delta;
            assert_ne!(run(&c.g, &none, k), want, "k {k}: the change moves the host path");
            assert_eq!(run(&c.g, &copy, k), want, "k {k}: the MSM read the host scalars");
            c.g[k] -= delta;
        }
        let other = c.g.clone();
        let copy = DeviceCopy::new(d_g, &c.g);
        let fallbacks = stats().host_fallbacks;
        assert_eq!(run(&other, &copy, 0), run(&c.g, &none, 0), "a copy of other coefficients is not read");
        assert_eq!(stats().host_fallbacks, fallbacks + 1);
    }

    /// `a` and `g` from the device, `b` uploaded, as the opening at gamma mixes
    /// them with host terms.
    #[test]
    fn accumulate_open_reads_device_terms() {
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        if !one_gpu() {
            return;
        }
        let rng = &mut TestRng::default();
        let mut c = chain(rng, LG, 1 << LG);
        let [d_a, _, d_g] = c.kept.take().expect("one GPU keeps the outputs");
        let out_len = c.a.len();
        let bases = bases(rng, out_len - 1);
        let (z, coeffs) = (Fr::rand(rng), [Fr::rand(rng), Fr::rand(rng), Fr::rand(rng)]);
        let none = DeviceCopy::default();
        let delta = Fr::rand(rng);
        let open = |a: &[Fr], g: &[Fr], copies: [&DeviceCopy; 2]| {
            let terms = [(coeffs[0], a, copies[0]), (coeffs[1], &c.b[..], &none), (coeffs[2], g, copies[1])];
            accumulate_open::<G1Affine, G1Projective, Fr>(out_len, &terms, &z, &bases)
                .map_err(|e| e.code)
                .expect("the opening runs")
                .to_affine()
        };
        let want = open(&c.a, &c.g, [&none, &none]);
        let (copy_a, copy_g) = (DeviceCopy::new(d_a, &c.a), DeviceCopy::new(d_g, &c.g));
        let reads = stats().device_reads;
        assert_eq!(open(&c.a, &c.g, [&copy_a, &copy_g]), want);
        assert_eq!(stats().device_reads, reads + 2, "both device terms were read");
        c.a[3] += delta;
        c.g[7] += delta;
        assert_ne!(open(&c.a, &c.g, [&none, &none]), want, "the change moves the host path");
        assert_eq!(open(&c.a, &c.g, [&copy_a, &copy_g]), want, "the opening read the host terms");
    }

    #[test]
    fn a_device_copy_serializes_as_nothing() {
        use snarkvm_utilities::{CanonicalDeserialize, CanonicalSerialize, Compress};
        let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        if !one_gpu() {
            return;
        }
        let c = chain(&mut TestRng::default(), LG, 1 << LG);
        let [.., d_g] = c.kept.expect("one GPU keeps the outputs");
        let copy = DeviceCopy::new(d_g, &c.g);
        let mut bytes = Vec::new();
        copy.serialize_with_mode(&mut bytes, Compress::Yes).unwrap();
        assert!(bytes.is_empty() && copy.serialized_size(Compress::No) == 0);
        assert!(DeviceCopy::deserialize_compressed(&[][..]).unwrap() == copy, "any two compare equal");
        assert_eq!(format!("{copy:?}"), format!("DevicePoly({})", c.g.len()));
    }
}
