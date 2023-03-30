use crate::field::{element::FieldElement, traits::IsTwoAdicField};

/// In-Place Radix-2 NR DIT FFT algorithm over a slice of two-adic field elements.
/// It's required that the twiddle factors are in bit-reverse order. Else this function will not
/// return fourier transformed values.
/// Also the input size needs to be a power of two.
/// It's recommended to use the current safe abstractions instead of this function.
///
/// Performs a fast fourier transform with the next attributes:
/// - In-Place: an auxiliary vector of data isn't needed for the algorithm.
/// - Radix-2: the algorithm halves the problem size log(n) times.
/// - NR: natural to reverse order, meaning that the input is naturally ordered and the output will
/// be bit-reversed ordered.
/// - DIT: decimation in time
pub fn in_place_nr_2radix_fft<F>(input: &mut [FieldElement<F>], twiddles: &[FieldElement<F>])
where
    F: IsTwoAdicField,
{
    // divide input in groups, starting with 1, duplicating the number of groups in each stage.
    let mut group_count = 1;
    let mut group_size = input.len();

    // for each group, there'll be group_size / 2 butterflies.
    // a butterfly is the atomic operation of a FFT, e.g: (a, b) = (a + wb, a - wb).
    // The 0.5 factor is what gives FFT its performance, it recursively halves the problem size
    // (group size).

    while group_count < input.len() {
        #[allow(clippy::needless_range_loop)] // the suggestion would obfuscate a bit the algorithm
        for group in 0..group_count {
            let first_in_group = group * group_size;
            let first_in_next_group = first_in_group + group_size / 2;

            let w = &twiddles[group]; // a twiddle factor is used per group

            for i in first_in_group..first_in_next_group {
                let y0 = &input[i] + w * &input[i + group_size / 2];
                let y1 = &input[i] - w * &input[i + group_size / 2];

                input[i] = y0;
                input[i + group_size / 2] = y1;
            }
        }
        group_count *= 2;
        group_size /= 2;
    }
}

/// In-Place Radix-2 RN DIT FFT algorithm over a slice of two-adic field elements.
/// It's required that the twiddle factors are naturally ordered (so w[i] = w^i). Else this
/// function will not return fourier transformed values.
/// Also the input size needs to be a power of two.
/// It's recommended to use the current safe abstractions instead of this function.
///
/// Performs a fast fourier transform with the next attributes:
/// - In-Place: an auxiliary vector of data isn't needed for storing the results.
/// - Radix-2: the algorithm halves the problem size log(n) times.
/// - RN: reverse to natural order, meaning that the input is bit-reversed ordered and the output will
/// be naturally ordered.
/// - DIT: decimation in time
#[allow(dead_code)]
pub fn in_place_rn_2radix_fft<F>(input: &mut [FieldElement<F>], twiddles: &[FieldElement<F>])
where
    F: IsTwoAdicField,
{
    // divide input in groups, starting with 1, duplicating the number of groups in each stage.
    let mut group_count = 1;
    let mut group_size = input.len();

    // for each group, there'll be group_size / 2 butterflies.
    // a butterfly is the atomic operation of a FFT, e.g: (a, b) = (a + wb, a - wb).
    // The 0.5 factor is what gives FFT its performance, it recursively halves the problem size
    // (group size).

    while group_count < input.len() {
        let step_to_next = 2 * group_count; // next butterfly in the group
        let step_to_last = step_to_next * (group_size / 2 - 1);

        for group in 0..group_count {
            for i in (group..=group + step_to_last).step_by(step_to_next) {
                let w = &twiddles[group * group_size / 2];
                let y0 = &input[i] + w * &input[i + group_count];
                let y1 = &input[i] - w * &input[i + group_count];

                input[i] = y0;
                input[i + group_count] = y1;
            }
        }
        group_count *= 2;
        group_size /= 2;
    }
}

#[cfg(test)]
mod tests {
    use crate::fft::helpers::log2;
    use crate::field::test_fields::u64_test_field::U64TestField;
    use crate::polynomial::Polynomial;
    use crate::{fft::bit_reversing::in_place_bit_reverse_permute, field::traits::RootsConfig};
    use proptest::{collection, prelude::*};

    use super::*;

    type F = U64TestField;
    type FE = FieldElement<F>;

    prop_compose! {
        fn powers_of_two(max_exp: u8)(exp in 1..max_exp) -> usize { 1 << exp }
        // max_exp cannot be multiple of the bits that represent a usize, generally 64 or 32.
        // also it can't exceed the test field's two-adicity.
    }
    prop_compose! {
        fn field_element()(num in any::<u64>().prop_filter("Avoid null coefficients", |x| x != &0)) -> FE {
            FE::from(num)
        }
    }
    prop_compose! {
        fn field_vec(max_exp: u8)(vec in collection::vec(field_element(), 2..1<<max_exp).prop_filter("Avoid polynomials of size not power of two", |vec| vec.len().is_power_of_two())) -> Vec<FE> {
            vec
        }
    }

    proptest! {
        // Property-based test that ensures NR Radix-2 FFT gives same result as a naive polynomial evaluation.
        #[test]
        fn test_nr_2radix_fft_matches_naive_eval(coeffs in field_vec(8)) {
            let expected = dft(&coeffs);

            let order = log2(coeffs.len()).unwrap();
            let twiddles = F::get_twiddles(order, RootsConfig::BitReverse).unwrap();

            let mut result = coeffs.clone();
            in_place_nr_2radix_fft(&mut result, &twiddles[..]);
            in_place_bit_reverse_permute(&mut result);

            prop_assert_eq!(expected, result);
        }
    }

    proptest! {
        // Property-based test that ensures RN Radix-2 FFT gives same result as a naive polynomial evaluation.
        #[test]
        fn test_rn_2radix_fft_matches_naive_eval(coeffs in field_vec(8)) {
            let expected = dft(&coeffs);

            let order = log2(coeffs.len()).unwrap();
            let twiddles = F::get_twiddles(order, RootsConfig::Natural).unwrap();

            let mut result = coeffs;
            in_place_bit_reverse_permute(&mut result[..]);
            in_place_rn_2radix_fft(&mut result, &twiddles[..]);

            prop_assert_eq!(result, expected);
        }
    }

    /// Calculates the (non-unitary) Discrete Fourier Transform of `input` via the DFT matrix.
    fn dft<F: IsTwoAdicField>(input: &[FieldElement<F>]) -> Vec<FieldElement<F>> {
        let n = input.len();
        let order = log2(n).unwrap();

        let twiddles = F::get_powers_of_primitive_root(order, n, RootsConfig::Natural).unwrap();

        let mut output = Vec::with_capacity(n);
        for row in 0..n {
            let mut sum = FieldElement::zero();

            for col in 0..n {
                let i = (row * col) % n; // w^i = w^(i mod n)
                sum = sum + input[col].clone() * twiddles[i].clone();
            }

            output.push(sum);
        }

        output
    }

    proptest! {
        // Property-based test that ensures dft() gives same result as a naive polynomial evaluation.
        #[test]
        fn test_dft_same_as_eval(coeffs in field_vec(8)) {
            let dft = dft(&coeffs);

            let poly = Polynomial::new(&coeffs[..]);
            let order = log2(coeffs.len()).unwrap();
            let twiddles = F::get_powers_of_primitive_root(order, coeffs.len(), RootsConfig::Natural).unwrap();
            let evals: Vec<FE> = twiddles.iter().map(|x| poly.evaluate(x)).collect();

            prop_assert_eq!(evals, dft);
        }
    }
}
