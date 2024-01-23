use madara_prover_common::models::{FriParameters, ProverParameters, StarkParameters};

/// Implements ceil(log2(x)).
fn ceil_log2(x: u32) -> u32 {
    let mut log = x.ilog2();
    if !x.is_power_of_two() {
        log += 1;
    }
    log
}

/// Computes the FRI steps list based on the number of Cairo steps of the program.
///
/// This computation is based on the documentation of the Stone prover:
/// # log₂(#steps) + 4 = log₂(last_layer_degree_bound) + ∑fri_step_list
/// # log₂(#steps) = log₂(last_layer_degree_bound) + ∑fri_step_list - 4
/// # ∑fri_step_list = log₂(#steps) + 4 - log₂(last_layer_degree_bound)
///
/// * `nb_steps`: Number of Cairo steps of the program.
/// * `last_layer_degree_bound`: Last layer degree bound.
///
/// Returns The FRI steps list.
pub fn compute_fri_steps(nb_steps: u32, last_layer_degree_bound: u32) -> Vec<u32> {
    let nb_steps_log = ceil_log2(nb_steps);
    let last_layer_degree_bound_log = ceil_log2(last_layer_degree_bound);

    let sigma_fri_step_list = nb_steps_log + 4 - last_layer_degree_bound_log;
    let quotient = (sigma_fri_step_list / 4) as usize;
    let remainder = sigma_fri_step_list % 4;

    let mut fri_steps = vec![4; quotient];
    if remainder > 0 {
        fri_steps.push(remainder);
    }

    fri_steps
}

/// Generates prover parameters based on program parameters.
///
/// * `nb_steps`: Number of Cairo steps of the program.
/// * `last_layer_degree_bound`: Last layer degree bound.
pub fn generate_prover_parameters(nb_steps: u32, last_layer_degree_bound: u32) -> ProverParameters {
    let fri_steps = compute_fri_steps(nb_steps, last_layer_degree_bound);
    ProverParameters {
        field: "PrimeField0".to_string(),
        stark: StarkParameters {
            fri: FriParameters {
                fri_step_list: fri_steps,
                last_layer_degree_bound,
                n_queries: 18,
                proof_of_work_bits: 24,
            },
            log_n_cosets: 4,
        },
        use_extension_field: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(2, 1)]
    #[case(32, 5)]
    #[case(1000, 10)]
    #[case(524288, 19)]
    fn test_ceil_log2(#[case] x: u32, #[case] expected: u32) {
        let log = ceil_log2(x);
        assert_eq!(log, expected);
    }

    #[rstest]
    #[case(32768, vec ! [4, 4, 4, 1])]
    #[case(524288, vec ! [4, 4, 4, 2])]
    #[case(768, vec ! [4, 4])]
    fn test_compute_fri_step_list(#[case] nb_steps: u32, #[case] expected: Vec<u32>) {
        let last_layer_degree_bound = 64;
        let step_list = compute_fri_steps(nb_steps, last_layer_degree_bound);
        assert_eq!(step_list, expected);
    }
}
