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

#include "snarkvm.cu"

#ifndef __CUDA_ARCH__

// Lazy instantiation of snarkvm_t
class snarkvm_singleton_t {
    bool failed = false;
    snarkvm_t *snarkvm = nullptr;

public:
    snarkvm_singleton_t() {}
    ~snarkvm_singleton_t() {
        delete snarkvm;
        snarkvm = nullptr;
    }
    bool ok() {
        if (!failed && snarkvm == nullptr) {
            // SNP TODO: max domain size?
            snarkvm = new snarkvm_t(17);
            if (snarkvm == nullptr) {
                failed = true;
            }
        }
        return snarkvm != nullptr;
    }
    snarkvm_t* operator->() {
        assert (ok());
        return snarkvm;
    }
};
snarkvm_singleton_t snarkvm_g;
                                         
#ifndef __CUDA_ARCH__

extern "C" {
    RustError snarkvm_ntt(fr_t* inout, uint32_t lg_domain_size,
                          NTT::InputOutputOrder ntt_order, NTT::Direction ntt_direction,
                          NTT::Type ntt_type)
    {
        if (!snarkvm_g.ok()) {
            return RustError{cudaErrorMemoryAllocation};
        }
        return snarkvm_g->NTT(inout, inout, lg_domain_size, ntt_order,
                              ntt_direction, ntt_type);
    }

    RustError snarkvm_polymul(fr_t* out,
                              size_t pcount, fr_t** polynomials, size_t* plens,
                              size_t ecount, fr_t** evaluations, size_t* elens,
                              uint32_t lg_domain_size) {
        if (!snarkvm_g.ok()) {
            return RustError{cudaErrorMemoryAllocation};
        }
        return snarkvm_g->PolyMul(out,
                                  pcount, polynomials, plens,
                                  ecount, evaluations, elens,
                                  lg_domain_size);
    }

    RustError snarkvm_msm(point_t* out, const affine_t points[], size_t npoints,
                          const scalar_t scalars[], size_t ffi_affine_size) {
        if (!snarkvm_g.ok()) {
            return RustError{cudaErrorMemoryAllocation};
        }
        return snarkvm_g->MSM(out, points, npoints, scalars, ffi_affine_size);
    }
}
#endif // __CUDA_ARCH__

#endif
