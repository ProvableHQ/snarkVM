// Elementwise kernels for `snarkvm.cu`.
//
// A separate translation unit: ptxas optimises per module, so a kernel added to
// `snarkvm_api.cu`, even one never called, perturbs register allocation for the
// NTT and MSM kernels there.

#include <cuda.h>

#if defined(FEATURE_BLS12_381)
# include <ff/bls12-381.hpp>
#elif defined(FEATURE_BLS12_377)
# include <ff/bls12-377.hpp>
#elif defined(FEATURE_BN254)
# include <ff/alt_bn128.hpp>
#else
# error "no FEATURE"
#endif

#include <util/exception.cuh>
#include <util/gpu_t.cuh>
#include <util/rusterror.h>

// Copies `npoints` packed records of `src_words` words into slots of
// `dst_words`, zeroing each slot's tail. One thread per destination word, so
// both sides are read and written in order.
__global__ void d_widen(uint64_t* dst, const uint64_t* src, size_t npoints,
                        size_t src_words, size_t dst_words)
{
    size_t total = npoints * dst_words;
    size_t stride = (size_t)gridDim.x * blockDim.x;
    for (size_t i = (size_t)blockIdx.x * blockDim.x + threadIdx.x; i < total; i += stride) {
        size_t point = i / dst_words;
        size_t word = i - point * dst_words;
        dst[i] = word < src_words ? src[point * src_words + word] : 0;
    }
}

#ifndef __CUDA_ARCH__

// Launcher for d_widen, kept in this translation unit for the reason at the
// top of this file.
void widen_points_device(const stream_t& stream, int sm_count, uint64_t* d_dst,
                         const uint64_t* d_src, size_t npoints,
                         size_t src_words, size_t dst_words)
{
    size_t total = npoints * dst_words;
    int block = 256;
    int grid = (int)std::min((total + block - 1) / block, (size_t)sm_count * 8);
    d_widen<<<grid, block, 0, stream>>>(d_dst, d_src, npoints, src_words, dst_words);
    CUDA_OK(cudaGetLastError());
}

#endif
