use cairo_vm::cairo_run::CairoRunConfig;
use cairo_vm::hint_processor::hint_processor_definition::HintProcessor;
use cairo_vm::types::errors::program_errors::ProgramError;
use cairo_vm::types::program::Program;
use cairo_vm::vm::errors::cairo_run_errors::CairoRunError;
use cairo_vm::vm::errors::vm_exception::VmException;
use cairo_vm::vm::runners::cairo_runner::CairoRunner;
use cairo_vm::vm::security::verify_secure_runner;
use std::any::Any;
use std::collections::HashMap;
use tonic::{Request, Response, Status};

use stone_prover_sdk::error::ProverError;
use stone_prover_sdk::models::{Layout, Proof, ProverConfig, ProverWorkingDirectory};

use crate::services::common::{
    call_prover, format_prover_error, get_prover_parameters, verify_and_annotate_proof,
};
use crate::services::starknet_prover::starknet_prover_proto::starknet_prover_server::StarknetProver;
use crate::services::starknet_prover::starknet_prover_proto::{
    StarknetExecutionRequest, StarknetProverResponse,
};
use stone_prover_sdk::cairo_vm::run_bootloader_in_proof_mode;

pub mod starknet_prover_proto {
    tonic::include_proto!("starknet_prover");
}

const BOOTLOADER_PROGRAM: &[u8] =
    include_bytes!("../../../test-cases/cases/bootloader/bootloader.json");

// Copied from cairo_run.rs and adapted to support injecting the bootloader input.
// TODO: check if modifying CairoRunConfig to specify custom variables is accepted upstream.
pub fn cairo_run(
    program: &Program,
    cairo_run_config: &CairoRunConfig,
    hint_executor: &mut dyn HintProcessor,
    variables: HashMap<String, Box<dyn Any>>,
) -> Result<CairoRunner, CairoRunError> {
    let secure_run = cairo_run_config
        .secure_run
        .unwrap_or(!cairo_run_config.proof_mode);

    let allow_missing_builtins = cairo_run_config.allow_missing_builtins.unwrap_or(false);

    let mut cairo_runner = CairoRunner::new(
        program,
        cairo_run_config.layout,
        cairo_run_config.proof_mode,
        allow_missing_builtins,
    )?;
    for (key, value) in variables {
        cairo_runner.exec_scopes.insert_box(&key, value);
    }

    let end = cairo_runner.initialize(allow_missing_builtins)?;
    // check step calculation

    cairo_runner
        .run_until_pc(end, hint_executor)
        .map_err(|err| VmException::from_vm_error(&cairo_runner, err))?;
    cairo_runner.end_run(cairo_run_config.disable_trace_padding, false, hint_executor)?;
    cairo_runner.read_return_values(allow_missing_builtins)?;
    if cairo_run_config.proof_mode {
        cairo_runner.finalize_segments()?;
    }
    if secure_run {
        verify_secure_runner(&cairo_runner, true, None)?;
    }
    cairo_runner.relocate(cairo_run_config.relocate_mem)?;

    Ok(cairo_runner)
}

#[derive(thiserror::Error, Debug)]
enum BootloaderTaskError {
    #[error("Failed to read program: {0}")]
    Program(#[from] ProgramError),

    #[error("Failed to read PIE: {0}")]
    Pie(#[from] std::io::Error),
}

/// Formats the output of the prover subprocess into the server response.
fn format_prover_result(
    prover_result: Result<(Proof, ProverWorkingDirectory), ProverError>,
) -> Result<StarknetProverResponse, Status> {
    match prover_result {
        Ok((proof, _)) => serde_json::to_string(&proof)
            .map(|proof_str| StarknetProverResponse { proof: proof_str })
            .map_err(|_| Status::internal("Could not parse the proof returned by the prover")),
        Err(e) => Err(format_prover_error(e)),
    }
}

#[derive(Debug, Default)]
pub struct StarknetProverService {}

#[tonic::async_trait]
impl StarknetProver for StarknetProverService {
    async fn execute_and_prove(
        &self,
        request: Request<StarknetExecutionRequest>,
    ) -> Result<Response<StarknetProverResponse>, Status> {
        let StarknetExecutionRequest {
            programs,
            pies,
            split_proof,
        } = request.into_inner();

        let bootloader_program = Program::from_bytes(BOOTLOADER_PROGRAM, Some("main"))
            .map_err(|e| Status::internal(format!("Failed to load bootloader program: {}", e)))?;
        let prover_config = ProverConfig::default();

        let bootloader_tasks = stone_prover_sdk::cairo_vm::make_bootloader_tasks(&programs, &pies)
            .map_err(|e| {
                Status::invalid_argument(format!("Could not parse programs/PIEs: {}", e))
            })?;

        let execution_artifacts = run_bootloader_in_proof_mode(
            &bootloader_program,
            bootloader_tasks,
            Some(Layout::StarknetWithKeccak),
            None,
            None,
        )
        .map_err(|e| Status::internal(format!("Failed to run bootloader: {e}")))?;

        let prover_parameters =
            get_prover_parameters(None, execution_artifacts.public_input.n_steps)?;

        let (mut proof, mut working_dir) =
            call_prover(&execution_artifacts, &prover_config, &prover_parameters)
                .await
                .map_err(format_prover_error)?;

        // If split proof was requested, build it
        if split_proof {
            verify_and_annotate_proof(&mut proof, &mut working_dir).await?;
        };

        format_prover_result(Ok((proof, working_dir))).map(Response::new)
    }
}
