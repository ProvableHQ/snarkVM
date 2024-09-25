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

use core::{
    fmt::{Debug, Display, Formatter},
    ops::{Add, Mul, Sub},
};

pub type Constant = Measurement<u64>;
pub type Public = Measurement<u64>;
pub type Private = Measurement<u64>;
pub type Constraints = Measurement<u64>;

/// A helper struct for tracking the number of constants, public inputs, private inputs, and constraints.
#[derive(Copy, Clone, Debug)]
pub struct Count(pub Constant, pub Public, pub Private, pub Constraints);

impl Count {
    /// Returns a new `Count` whose constituent metrics are all `Exact`.
    pub const fn zero() -> Self {
        Count(Measurement::Exact(0), Measurement::Exact(0), Measurement::Exact(0), Measurement::Exact(0))
    }

    /// Returns a new `Count` whose constituent metrics are all `Exact`.
    pub const fn is(num_constants: u64, num_public: u64, num_private: u64, num_constraints: u64) -> Self {
        Count(
            Measurement::Exact(num_constants),
            Measurement::Exact(num_public),
            Measurement::Exact(num_private),
            Measurement::Exact(num_constraints),
        )
    }

    /// Returns a new `Count` whose constituent metrics are all inclusive `UpperBound`.
    pub const fn less_than(num_constants: u64, num_public: u64, num_private: u64, num_constraints: u64) -> Self {
        Count(
            Measurement::UpperBound(num_constants),
            Measurement::UpperBound(num_public),
            Measurement::UpperBound(num_private),
            Measurement::UpperBound(num_constraints),
        )
    }

    /// Returns `true` if all constituent metrics match.
    pub fn matches(&self, num_constants: u64, num_public: u64, num_private: u64, num_constraints: u64) -> bool {
        self.0.matches(num_constants)
            && self.1.matches(num_public)
            && self.2.matches(num_private)
            && self.3.matches(num_constraints)
    }
}

impl Add for Count {
    type Output = Count;

    /// Adds the `Count` to another `Count` by summing its constituent metrics.
    fn add(self, other: Count) -> Self::Output {
        Count(self.0 + other.0, self.1 + other.1, self.2 + other.2, self.3 + other.3)
    }
}

impl Mul<u64> for Count {
    type Output = Count;

    /// Scales the `Count` by a `u64`.
    fn mul(self, other: u64) -> Self::Output {
        Count(self.0 * other, self.1 * other, self.2 * other, self.3 * other)
    }
}

impl Mul<Count> for u64 {
    type Output = Count;

    /// Scales the `Count` by a `u64`.
    fn mul(self, other: Count) -> Self::Output {
        other * self
    }
}

impl Display for Count {
    fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
        write!(f, "Constants: {}, Public: {}, Private: {}, Constraints: {}", self.0, self.1, self.2, self.3)
    }
}

/// A `Measurement` is a quantity that can be measured.
/// The variants of the `Measurement` defines a condition associated with the measurable quantity.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Measurement<V: Copy + Debug + Display + Ord + Add<Output = V> + Sub<Output = V> + Mul<Output = V>> {
    Exact(V),
    Range(V, V),
    UpperBound(V),
}

impl<V: Copy + Debug + Display + Ord + Add<Output = V> + Sub<Output = V> + Mul<Output = V>> Measurement<V> {
    /// Returns `true` if the value matches the metric.
    ///
    /// For an `Exact` metric, `value` must be equal to the exact value defined by the metric.
    /// For a `Range` metric, `value` must be satisfy lower bound and the upper bound.
    /// For an `UpperBound` metric, `value` must be satisfy the upper bound.
    pub fn matches(&self, candidate: V) -> bool {
        let outcome = match self {
            Measurement::Exact(expected) => *expected == candidate,
            Measurement::Range(lower, upper) => candidate >= *lower && candidate <= *upper,
            Measurement::UpperBound(bound) => candidate <= *bound,
        };

        if !outcome {
            eprintln!("Metrics claims the count should be {self:?}, found {candidate:?} during synthesis");
        }

        outcome
    }
}

impl<V: Copy + Debug + Display + Ord + Add<Output = V> + Sub<Output = V> + Mul<Output = V>> Add for Measurement<V> {
    type Output = Measurement<V>;

    /// Adds two variants of `Measurement` together, returning the newly-summed `Measurement`.
    fn add(self, other: Measurement<V>) -> Self::Output {
        match (self, other) {
            // `Exact` + `Exact` => `Exact`
            (Measurement::Exact(exact_a), Measurement::Exact(exact_b)) => Measurement::Exact(exact_a + exact_b),
            // `Range` + `Range` => `Range`
            (Measurement::Range(lower_a, upper_a), Measurement::Range(lower_b, upper_b)) => {
                Measurement::Range(lower_a + lower_b, upper_a + upper_b)
            }
            // `UpperBound` + `UpperBound` => `UpperBound`
            (Measurement::UpperBound(upper_a), Measurement::UpperBound(upper_b)) => {
                Measurement::UpperBound(upper_a + upper_b)
            }
            // `Exact` + `Range` => `Range`
            // `Range` + `Exact` => `Range`
            (Measurement::Exact(exact), Measurement::Range(lower, upper))
            | (Measurement::Range(lower, upper), Measurement::Exact(exact)) => {
                Measurement::Range(exact + lower, exact + upper)
            }
            // `Exact` + `UpperBound` => `UpperBound`
            // `UpperBound` + `Exact` => `UpperBound`
            (Measurement::Exact(exact), Measurement::UpperBound(upper))
            | (Measurement::UpperBound(upper), Measurement::Exact(exact)) => Measurement::UpperBound(exact + upper),
            // `Range` + `UpperBound` => `Range`
            // `UpperBound` + `Range` => `Range`
            (Measurement::Range(lower, upper_a), Measurement::UpperBound(upper_b))
            | (Measurement::UpperBound(upper_a), Measurement::Range(lower, upper_b)) => {
                Measurement::Range(lower, upper_a + upper_b)
            }
        }
    }
}

impl<V: Copy + Debug + Display + Ord + Add<Output = V> + Sub<Output = V> + Mul<Output = V>> Mul<V> for Measurement<V> {
    type Output = Measurement<V>;

    /// Scales the `Measurement` by a value.
    fn mul(self, other: V) -> Self::Output {
        match self {
            Measurement::Exact(value) => Measurement::Exact(value * other),
            Measurement::Range(lower, upper) => Measurement::Range(lower * other, upper * other),
            Measurement::UpperBound(bound) => Measurement::UpperBound(bound * other),
        }
    }
}

impl<V: Copy + Debug + Display + Ord + Add<Output = V> + Sub<Output = V> + Mul<Output = V>> Display for Measurement<V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Measurement::Exact(value) => write!(f, "{}", value),
            Measurement::Range(lower, upper) => write!(f, "[{}, {}]", lower, upper),
            Measurement::UpperBound(bound) => write!(f, "<={}", bound),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use snarkvm_utilities::{TestRng, Uniform};

    const ITERATIONS: u64 = 1024;

    #[test]
    fn test_exact_matches() {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Generate a random `Measurement` and candidate value.
            let value = u32::rand(&mut rng) as u64;
            let candidate = u32::rand(&mut rng) as u64;
            let metric = Measurement::Exact(value);

            // Check that the metric is only satisfied if the candidate is equal to the value.
            assert!(metric.matches(value));
            if candidate == value {
                assert!(metric.matches(candidate));
            } else {
                assert!(!metric.matches(candidate));
            }
        }
    }

    #[test]
    fn test_upper_matches() {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Generate a random `Measurement::UpperBound` and candidate value.
            let upper = u32::rand(&mut rng) as u64;
            let candidate = u32::rand(&mut rng) as u64;
            let metric = Measurement::UpperBound(upper);

            // Check that the metric is only satisfied if the candidate is less than upper.
            assert!(metric.matches(upper));
            if candidate <= upper {
                assert!(metric.matches(candidate));
            } else {
                assert!(!metric.matches(candidate));
            }
        }
    }

    #[test]
    fn test_range_matches() {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            // Generate a random `Measurement::UpperBound` and candidate value.
            let first_bound = u32::rand(&mut rng) as u64;
            let second_bound = u32::rand(&mut rng) as u64;
            let candidate = u32::rand(&mut rng) as u64;
            let (metric, lower, upper) = if first_bound <= second_bound {
                (Measurement::Range(first_bound, second_bound), first_bound, second_bound)
            } else {
                (Measurement::Range(second_bound, first_bound), second_bound, first_bound)
            };

            // Check that the metric is only satisfied if the candidate is less than upper.
            assert!(metric.matches(lower));
            assert!(metric.matches(upper));
            if lower <= candidate && candidate <= upper {
                assert!(metric.matches(candidate));
            } else {
                assert!(!metric.matches(candidate));
            }
        }
    }

    // Test addition.

    #[test]
    fn test_exact_plus_exact() {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            let first = u32::rand(&mut rng) as u64;
            let second = u32::rand(&mut rng) as u64;
            let candidate = u32::rand(&mut rng) as u64;

            let a = Measurement::Exact(first);
            let b = Measurement::Exact(second);
            let c = a + b;

            assert!(c.matches(first + second));
            if candidate == first + second {
                assert!(c.matches(candidate));
            } else {
                assert!(!c.matches(candidate));
            }
        }
    }

    #[test]
    fn test_exact_plus_upper() {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            let first = u32::rand(&mut rng) as u64;
            let second = u32::rand(&mut rng) as u64;
            let candidate = u32::rand(&mut rng) as u64;

            let a = Measurement::Exact(first);
            let b = Measurement::UpperBound(second);
            let c = a + b;

            assert!(c.matches(first + second));
            if candidate <= first + second {
                assert!(c.matches(candidate));
            } else {
                assert!(!c.matches(candidate));
            }
        }
    }

    #[test]
    fn test_exact_plus_range() {
        let mut rng = TestRng::default();

        let value = u32::rand(&mut rng) as u64;
        let first_bound = u32::rand(&mut rng) as u64;
        let second_bound = u32::rand(&mut rng) as u64;
        let candidate = u32::rand(&mut rng) as u64;

        let a = Measurement::Exact(value);
        let (b, lower, upper) = if first_bound <= second_bound {
            (Measurement::Range(first_bound, second_bound), first_bound, second_bound)
        } else {
            (Measurement::Range(second_bound, first_bound), second_bound, first_bound)
        };
        let c = a + b;

        assert!(c.matches(value + lower));
        assert!(c.matches(value + upper));
        if value + lower <= candidate && candidate <= value + upper {
            assert!(c.matches(candidate));
        } else {
            assert!(!c.matches(candidate));
        }
    }

    #[test]
    fn test_range_plus_exact() {
        let mut rng = TestRng::default();

        let value = u32::rand(&mut rng) as u64;
        let first_bound = u32::rand(&mut rng) as u64;
        let second_bound = u32::rand(&mut rng) as u64;
        let candidate = u32::rand(&mut rng) as u64;

        let (a, lower, upper) = if first_bound <= second_bound {
            (Measurement::Range(first_bound, second_bound), first_bound, second_bound)
        } else {
            (Measurement::Range(second_bound, first_bound), second_bound, first_bound)
        };
        let b = Measurement::Exact(value);
        let c = a + b;

        assert!(c.matches(value + lower));
        assert!(c.matches(value + upper));
        if value + lower <= candidate && candidate <= value + upper {
            assert!(c.matches(candidate));
        } else {
            assert!(!c.matches(candidate));
        }
    }

    #[test]
    fn test_range_plus_range() {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            let first = u32::rand(&mut rng) as u64;
            let second = u32::rand(&mut rng) as u64;
            let third = u32::rand(&mut rng) as u64;
            let fourth = u32::rand(&mut rng) as u64;
            let candidate = u32::rand(&mut rng) as u64;

            let (a, first_lower, first_upper) = if first <= second {
                (Measurement::Range(first, second), first, second)
            } else {
                (Measurement::Range(second, first), second, first)
            };
            let (b, second_lower, second_upper) = if third <= fourth {
                (Measurement::Range(third, fourth), third, fourth)
            } else {
                (Measurement::Range(fourth, third), fourth, third)
            };
            let c = a + b;

            assert!(c.matches(first_lower + second_lower));
            assert!(c.matches(first_upper + second_upper));
            if first_lower + second_lower <= candidate && candidate <= first_upper + second_upper {
                assert!(c.matches(candidate));
            } else {
                assert!(!c.matches(candidate));
            }
        }
    }

    #[test]
    fn test_range_plus_upper() {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            let first = u32::rand(&mut rng) as u64;
            let second = u32::rand(&mut rng) as u64;
            let third = u32::rand(&mut rng) as u64;
            let candidate = u32::rand(&mut rng) as u64;

            let (a, lower, upper) = if second <= third {
                (Measurement::Range(second, third), second, third)
            } else {
                (Measurement::Range(third, second), third, second)
            };
            let b = Measurement::UpperBound(first);
            let c = a + b;

            assert!(c.matches(lower));
            assert!(c.matches(first + upper));
            if lower <= candidate && candidate <= first + upper {
                assert!(c.matches(candidate));
            } else {
                assert!(!c.matches(candidate));
            }
        }
    }

    #[test]
    fn test_upper_plus_exact() {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            let first = u32::rand(&mut rng) as u64;
            let second = u32::rand(&mut rng) as u64;
            let candidate = u32::rand(&mut rng) as u64;

            let a = Measurement::UpperBound(second);
            let b = Measurement::Exact(first);
            let c = a + b;

            assert!(c.matches(first + second));
            if candidate <= first + second {
                assert!(c.matches(candidate));
            } else {
                assert!(!c.matches(candidate));
            }
        }
    }

    #[test]
    fn test_upper_plus_range() {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            let first = u32::rand(&mut rng) as u64;
            let second = u32::rand(&mut rng) as u64;
            let third = u32::rand(&mut rng) as u64;
            let candidate = u32::rand(&mut rng) as u64;

            let a = Measurement::UpperBound(first);
            let (b, lower, upper) = if second <= third {
                (Measurement::Range(second, third), second, third)
            } else {
                (Measurement::Range(third, second), third, second)
            };
            let c = a + b;

            assert!(c.matches(lower));
            assert!(c.matches(first + upper));
            if lower <= candidate && candidate <= first + upper {
                assert!(c.matches(candidate));
            } else {
                assert!(!c.matches(candidate));
            }
        }
    }

    #[test]
    fn test_upper_plus_upper() {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            let first = u32::rand(&mut rng) as u64;
            let second = u32::rand(&mut rng) as u64;
            let candidate = u32::rand(&mut rng) as u64;

            let a = Measurement::UpperBound(second);
            let b = Measurement::UpperBound(first);
            let c = a + b;

            assert!(c.matches(first + second));
            if candidate <= first + second {
                assert!(c.matches(candidate));
            } else {
                assert!(!c.matches(candidate));
            }
        }
    }

    // Test multiplication.

    #[test]
    fn test_exact_mul() {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            let start = u32::rand(&mut rng) as u64;
            let scalar = u32::rand(&mut rng) as u64;

            let expected = Measurement::Exact(start * scalar);
            let candidate = Measurement::Exact(start) * scalar;
            assert_eq!(candidate, expected);
        }
    }

    #[test]
    fn test_upper_bound_mul() {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            let start = u32::rand(&mut rng) as u64;
            let scalar = u32::rand(&mut rng) as u64;

            let expected = Measurement::UpperBound(start * scalar);
            let candidate = Measurement::UpperBound(start) * scalar;
            assert_eq!(candidate, expected);
        }
    }

    #[test]
    fn test_range_mul() {
        let mut rng = TestRng::default();

        for _ in 0..ITERATIONS {
            let start = u32::rand(&mut rng) as u64;
            let end = u32::rand(&mut rng) as u64;
            let scalar = u32::rand(&mut rng) as u64;

            let expected = Measurement::Range(start * scalar, end * scalar);
            let candidate = Measurement::Range(start, end) * scalar;
            assert_eq!(candidate, expected);
        }
    }
}
