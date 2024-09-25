// Copyright 2024 Aleo Network Foundation
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

#include <ntt/ntt.cuh>
#include "polynomial.cuh"

#include <ec/jacobian_t.hpp>
#include <ec/xyzz_t.hpp>

typedef jacobian_t<fp_t> point_t;
typedef xyzz_t<fp_t> bucket_t;
typedef bucket_t::affine_inf_t affine_t;
typedef fr_t scalar_t;

#define WBITS 11 // Suitable for ~2^16
#include <msm/pippenger.cuh>
#undef WBITS

// A simple way to allocate a host pointer without having to
// care about freeing it.
template<typename T> class host_ptr_t {
    T* h_ptr;
public:
    host_ptr_t(size_t nelems) : h_ptr(nullptr)
    {
        if (nelems) {
            CUDA_OK(cudaMallocHost(&h_ptr, nelems * sizeof(T)));
        }
    }
    ~host_ptr_t() { if (h_ptr) cudaFreeHost((void*)h_ptr); }

    inline operator const T*() const            { return h_ptr; }
    inline operator T*() const                  { return h_ptr; }
    inline operator void*() const               { return (void*)h_ptr; }
    inline const T& operator[](size_t i) const  { return h_ptr[i]; }
    inline T& operator[](size_t i)              { return h_ptr[i]; }
};

#ifndef __CUDA_ARCH__

#include <vector>
#include <chrono>
#include <atomic>

typedef std::chrono::high_resolution_clock Clock;

using namespace std;

class snarkvm_t {
    thread_pool_t pool;

    struct resource_t {
        int dev;
        int stream;
        resource_t(int _dev, int _stream) {
            dev = _dev;
            stream = _stream;
        }
    };
    channel_t<resource_t*> resources;
    
    // Host and device storage
    size_t                         allocated_elements;
    std::vector<dev_ptr_t<fr_t>*>  d_mem;
    std::vector<host_ptr_t<fr_t>*> h_mem;

    void free_memory() {
        allocated_elements = 0;
        for (size_t i = 0; i < ngpus(); i++) {
            if (d_mem[i] != nullptr) {
                delete d_mem[i];
                d_mem[i] = nullptr;
            }
            if (h_mem[i] != nullptr) {
                delete h_mem[i];
                h_mem[i] = nullptr;
            }
        }
    }

    fr_t *d_addr(uint32_t dev, uint32_t stream) {
        if (allocated_elements == 0) {
            assert(false);
            return nullptr;
        }
        fr_t* dmem = *d_mem[dev];
        return &dmem[allocated_elements * stream];
    }
    fr_t *h_addr(uint32_t dev, uint32_t stream) {
        if (allocated_elements == 0) {
            assert(false);
            return nullptr;
        }
        fr_t* hmem = *h_mem[dev];
        return &hmem[allocated_elements * stream];
    }
    
public:
    snarkvm_t(int max_lg_domain) : pool(ngpus()) {
        // Allocate storage for 4 polynomials, required by polynomial multiplication
        // Will be allocated per gpu per stream
        allocated_elements = ((size_t)1 << max_lg_domain) * 4;

        d_mem.resize(ngpus());
        h_mem.resize(ngpus());
        for (size_t i = 0; i < ngpus(); i++) {
            d_mem[i] = nullptr;
            h_mem[i] = nullptr;
        }
        try {
            for (size_t i = 0; i < ngpus(); i++) {
                auto& gpu = select_gpu(i);
                d_mem[i] = new dev_ptr_t<fr_t>(allocated_elements * gpu_t::FLIP_FLOP);
                h_mem[i] = new host_ptr_t<fr_t>(allocated_elements * gpu_t::FLIP_FLOP);
            }
        } catch (const cuda_error& e) {
            // Failed to allocate. Clean up any memory
            free_memory();
        }            

        // GPU resource allocation scheme
        for (size_t j = 0; j < gpu_t::FLIP_FLOP; j++) {
            for (size_t dev = 0; dev < ngpus(); dev++) {
                resources.send(new resource_t(dev, j));
            }
        }
    }
    ~snarkvm_t() {}

    RustError NTT(fr_t* out, fr_t* in,
                  uint32_t lg_domain_size,
                  NTT::InputOutputOrder ntt_order,
                  NTT::Direction ntt_direction,
                  NTT::Type ntt_type) {
        size_t domain_size = (size_t)1 << lg_domain_size;

        // Ensure enough sufficient memory
        if (allocated_elements < domain_size) {
            return RustError{cudaErrorMemoryAllocation};
        }

        resource_t* resource = resources.recv();
        int dev = resource->dev;
        auto& gpu = select_gpu(dev);
        int stream_idx = resource->stream;
        stream_t& stream = gpu[stream_idx];

        fr_t* h_mem = h_addr(dev, stream_idx);
        memcpy(h_mem, in, sizeof(fr_t) * domain_size);

        // Perform NTT
        RustError e = Polynomial::Base(gpu, h_mem, lg_domain_size,
                                       ntt_order, ntt_direction, ntt_type);
        if (e.code != cudaSuccess) {
            resources.send(resource);
            return e;
        }
        // Successful. Copy the result back
        memcpy(out, h_mem, sizeof(fr_t) * domain_size);
        resources.send(resource);
        return RustError{cudaSuccess};
    }

    RustError PolyMul(fr_t* out,
                      size_t pcount, fr_t** polynomials, size_t* plens,
                      size_t ecount, fr_t** evaluations, size_t* elens,
                      uint32_t lg_domain_size) {
        // domain_size is the size of the final polynomial
        size_t domain_size = (size_t)1 << lg_domain_size;

        // Corner cases
        if (pcount + ecount == 0) {
            return RustError{cudaSuccess};
        } else if (pcount + ecount == 1) {
            if (pcount == 1) {
                memcpy(out, polynomials[0], sizeof(fr_t) * plens[0]);
            } else {
                // Perform iNTT on the single evaluation
                memset((uint8_t*)out, 0, sizeof(fr_t) * domain_size);
                memcpy(out, evaluations[0], sizeof(fr_t) * elens[0]);
                return NTT(out, out, lg_domain_size,
                           NTT::InputOutputOrder::NN, NTT::Direction::inverse,
                           NTT::Type::standard);
            }
            return RustError{cudaSuccess};
        }
        
        // Ensure enough sufficient memory
        if (allocated_elements < 4 * domain_size) {
            return RustError{cudaErrorMemoryAllocation};
        }

        resource_t* resource = resources.recv();
        int dev = resource->dev;
        auto& gpu = select_gpu(dev);
        int stream_idx = resource->stream;
        stream_t& stream = gpu[stream_idx];

        // // Copy data to pinned staging buffer
        fr_t* h_mem0 = h_addr(dev, stream_idx);
        fr_t* h_mem1 = &h_mem0[domain_size];
        fr_t* h_mem2 = &h_mem0[domain_size * 2];
        fr_t* d_mem0 = d_addr(dev, stream_idx);
        fr_t* d_mem1 = &d_mem0[domain_size];
        fr_t* d_mem2 = &d_mem0[domain_size * 2];
        fr_t* d_mem3 = &d_mem0[domain_size * 3];
        RustError e = Polynomial::Mul(gpu, stream,
                                      h_mem0, h_mem1, h_mem2,
                                      d_mem0, d_mem1, d_mem2, d_mem3,
                                      pcount, polynomials, plens,
                                      ecount, evaluations, elens,
                                      lg_domain_size);
        if (e.code != cudaSuccess) {
            resources.send(resource);
            return e;
        }

        cudaDeviceSynchronize();
        memcpy(out, h_mem0, sizeof(fr_t) * domain_size);

        resources.send(resource);
        return RustError{cudaSuccess};
    }

    RustError MSM(point_t* out, const affine_t points[], size_t npoints,
                  const scalar_t scalars[], size_t ffi_affine_size) {
        // SNP TODO: cleanup
        // auto start = Clock::now();

        size_t gpu_count = min(ngpus(), npoints);
        point_t partial_sums[gpu_count];
        size_t bases_per_gpu = (npoints + gpu_count - 1) / gpu_count;
        channel_t<size_t> ch;
        RustError error = RustError{cudaSuccess};

        // Divide the MSM among the GPUs
        for (size_t i = 0; i < gpu_count; i++) {
            pool.spawn([&, i]() {
                int dev = i;
                select_gpu(dev);
                size_t start = i * bases_per_gpu;
                size_t sz = std::min(bases_per_gpu, npoints - start);

                // This is ugly, but we only know the size of the affine points in bytes
                const affine_t* pts = (affine_t*)(&((uint8_t*)points)[start * ffi_affine_size]);
                
                RustError ret;
                try {
                    msm_t<bucket_t, point_t, affine_t, scalar_t> msm(dev);
                    ret = msm.invoke(partial_sums[i], slice_t<affine_t>{pts, sz},
                                     &scalars[start], false, ffi_affine_size);
                } catch (const cuda_error& e) {
                    out->inf();
#ifdef TAKE_RESPONSIBILITY_FOR_ERROR_MESSAGE
                    ret = RustError{e.code(), e.what()};
#else
                    ret = RustError{e.code()};
#endif
                }
                if (ret.code != cudaSuccess) {
                    error = ret;
                }
                ch.send(i);
            });
        }
        size_t dev = ch.recv();
        *out = partial_sums[dev];
        for (size_t i = 0; i < gpu_count - 1; i++) {
            dev = ch.recv();
            point_t::dadd(*out, *out, partial_sums[dev]);
        }
        // auto end = Clock::now();
        // uint64_t dt = std::chrono::duration_cast<
        //     std::chrono::microseconds>(end - start).count();
        // printf("MSM size %ld took %ld us\n", npoints, dt);

        return error;
        
        // auto start = Clock::now();
        // auto res = mult_pippenger<bucket_t>(out, points, npoints, scalars,
        //                                     false, ffi_affine_size);
        // auto end = Clock::now();
        // uint64_t dt = std::chrono::duration_cast<
        //     std::chrono::microseconds>(end - start).count();
        // printf("MSM size %ld took %ld us\n", npoints, dt);
        // return res;
    }
};

#endif
