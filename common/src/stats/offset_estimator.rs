use std::time::{SystemTime, UNIX_EPOCH};

/// A simple Linear Congruential Generator (LCG) for generating pseudorandom numbers.
///
/// # Parameters
///
/// * `state: u64` - The current internal state of the generator.
/// * `a: u64` - The multiplier constant.
/// * `c: u64` - The increment constant.
/// * `m: u64` - The modulus constant.
///
/// # References
///
/// * D. H. Lehmer. "Mathematical methods in large-scale computing units".
///   Proceedings of a Second Symposium on Large Scale Digital Calculating Machinery;
///   Annals of the Computation Laboratory, Harvard Univ. 26 (1951): 141-146.
pub struct LcgRng {
    state: u64,
    a: u64,
    c: u64,
    m: u64,
}

impl LcgRng {
    pub fn new(seed: u64) -> Self {
        LcgRng {
            state: seed,
            a: 6364136223846793005,
            c: 1442695040888963407,
            m: u64::MAX,
        }
    }

    pub fn gen_range(&mut self, range: std::ops::Range<f64>) -> f64 {
        let random_u64 = self.next_u64();
        let random_f64 = random_u64 as f64 / u64::MAX as f64;
        range.start + random_f64 * (range.end - range.start)
    }

    fn next_u64(&mut self) -> u64 {
        self.state = (self.a.wrapping_mul(self.state).wrapping_add(self.c)) % self.m;
        self.state
    }
}

/// Estimates the alpha and beta parameters for the Gamma distribution based on the sample data provided.
fn estimate_alpha_beta_from_owd<'a, I>(x: I) -> (f64, f64)
where
    I: IntoIterator<Item = &'a f64>,
    I::IntoIter: ExactSizeIterator + Clone,
{
    let x_vec: Vec<_> = x.into_iter().cloned().collect();
    let n = x_vec.len() as f64;
    let sum_x = x_vec.iter().sum::<f64>();
    let mean_x = sum_x / n;
    let sum_sq_diff = x_vec.iter().map(|&xi| (xi - mean_x).powi(2)).sum::<f64>();
    let var_x = sum_sq_diff / (n - 1.0);
    let alpha = mean_x.powi(2) / var_x;
    let beta = var_x / mean_x;
    (alpha, beta)
}

/// Sorts the input values in ascending order and returns the sorted vector.
fn sort_values<'a, I>(values: I) -> Vec<f64>
where
    I: IntoIterator<Item = &'a f64>,
    I::IntoIter: ExactSizeIterator + Clone,
{
    let mut sorted_values: Vec<_> = values.into_iter().cloned().collect();
    sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sorted_values
}

/// Generates a random value drawn from a standard normal distribution using the Marsaglia polar method.
///
/// This function implements the Marsaglia polar method, an algorithm for generating
/// independent, standard normally distributed (Gaussian) random numbers.
/// /// References:
/// George Marsaglia. "Generating a Variable from the Tail of the Normal Distribution".
/// Technometrics, Vol. 6, No. 3 (Aug., 1964), pp. 101-102.
fn marsaglia_polar_sample(rng: &mut LcgRng) -> f64 {
    loop {
        let u: f64 = rng.gen_range(-1.0..1.0);
        let v: f64 = rng.gen_range(-1.0..1.0);
        let s = u * u + v * v;
        if s < 1.0 && s != 0.0 {
            let z0 = u * (-2.0 * s.ln() / s).sqrt();
            return z0;
        }
    }
}

/// Generates random values drawn from a Gamma distribution using the method described in:
///
/// George Marsaglia, Wai Wan Tsang. "A Simple Method for Generating Gamma Variables".
/// ACM Transactions on Mathematical Software, Vol. 26, No. 3, September 2000, Pages 363-372.
fn generate_random_gamma_values(alpha: f64, beta: f64, num_samples: usize, seed: u64) -> Vec<f64> {
    let mut rng = LcgRng::new(seed);
    (0..num_samples)
        .map(|_| {
            let d = alpha - 1.0 / 3.0;
            let c = (1.0 / 3.0) / d.sqrt();

            loop {
                let x = marsaglia_polar_sample(&mut rng);
                let v = 1.0 + c * x;
                if v <= 0.0 {
                    continue;
                }

                let v = v * v * v;
                let u = rng.gen_range(0.0..1.0);

                let x_squared = x * x;

                if u < 1.0 - 0.0331 * x_squared * x_squared
                    || u.ln() < 0.5 * x_squared + d * (1.0 - v + v.ln())
                {
                    break d * v * beta;
                }
            }
        })
        .collect()
}
/// Estimates the offset between two networked devices based on one-way delay time (OWD) measurements
/// using the method described in:
///
/// Edmar Mota-Garcia and Rogelio Hasimoto-Beltran: "A new model-based clock-offset approximation over IP networks"
/// Computer Communications, Volume 53, 2014, Pages 26-36, ISSN 0140-3664, https://doi.org/10.1016/j.comcom.2014.07.006.
pub fn estimate<I>(time_values: I) -> f64
where
    I: IntoIterator<Item = f64>,
{
    let time_values_vec: Vec<f64> = time_values.into_iter().collect();
    let n = time_values_vec.len();
    let (mut alpha, mut beta) = estimate_alpha_beta_from_owd(&time_values_vec);
    if beta > 1.5 {
        beta = 1.5;
    } else if beta < 0.1 {
        beta = 0.1;
    }
    if alpha > 4.0 {
        alpha = 4.0;
    } else if alpha < 1.0 {
        alpha = 1.0;
    }
    let random_values = generate_random_gamma_values(alpha, beta, n, get_time_based_seed());
    // sort random values
    let sorted = sort_values(&time_values_vec);
    let mut random_sorted = random_values;
    // sort in increasing order
    random_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

    estimate_offset(&sorted, &random_sorted)
}

/// Calculates the offset between the generated gamma values and the sorted time values.
///
/// Edmar Mota-Garcia and Rogelio Hasimoto-Beltran: "A new model-based clock-offset approximation over IP networks"
/// Computer Communications, Volume 53, 2014, Pages 26-36, ISSN 0140-3664, https://doi.org/10.1016/j.comcom.2014.07.006.
fn estimate_offset(x_sort: &[f64], y: &[f64]) -> f64 {
    let n = x_sort.len();
    let mut y_regression = Vec::new();
    let mut x_regression = Vec::new();

    // Create vectors for y and x_sort for use in linear regression (QQ-plot)
    for i in 0..n {
        let rank = (i + 1) as f64;
        let p_value = (rank - 0.5) / n as f64;
        y_regression.push(y[i]);
        x_regression.push(x_sort[i] - p_value);
    }

    // Perform linear regression to estimate the slope (beta) and intercept (gamma)
    let beta = {
        let x_mean = x_regression.iter().sum::<f64>() / x_regression.len() as f64;
        let y_mean = y_regression.iter().sum::<f64>() / y_regression.len() as f64;
        let numerator = x_regression
            .iter()
            .zip(y_regression.iter())
            .map(|(x, y)| (x - x_mean) * (y - y_mean))
            .sum::<f64>();
        let denominator = x_regression
            .iter()
            .map(|x| (x - x_mean).powi(2))
            .sum::<f64>();
        numerator / denominator
    };
    let gamma = {
        let y_mean = y_regression.iter().sum::<f64>() / y_regression.len() as f64;
        let x_mean = x_regression.iter().sum::<f64>() / x_regression.len() as f64;
        y_mean - beta * x_mean
    };

    // Find the point where the regression line crosses the x-axis (y = 0)

    // Return the estimated offset (x_cross).
    -gamma / beta
}

fn get_time_based_seed() -> u64 {
    let now = SystemTime::now();
    let duration = now.duration_since(UNIX_EPOCH).expect("Time went backwards");
    duration.as_secs() ^ duration.subsec_nanos() as u64
}
