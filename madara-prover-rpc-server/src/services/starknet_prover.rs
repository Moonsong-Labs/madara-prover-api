use std::any::Any;
use std::collections::HashMap;

use cairo_vm::cairo_run::CairoRunConfig;
use cairo_vm::hint_processor::builtin_hint_processor::bootloader::types::{
    BootloaderConfig, BootloaderInput, PackedOutput, SimpleBootloaderInput, Task, TaskSpec,
};
use cairo_vm::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor;
use cairo_vm::hint_processor::hint_processor_definition::HintProcessor;
use cairo_vm::types::errors::cairo_pie_error::CairoPieError;
use cairo_vm::types::errors::program_errors::ProgramError;
use cairo_vm::types::program::Program;
use cairo_vm::vm::errors::cairo_run_errors::CairoRunError;
use cairo_vm::vm::errors::vm_exception::VmException;
use cairo_vm::vm::runners::cairo_pie::CairoPie;
use cairo_vm::vm::runners::cairo_runner::CairoRunner;
use cairo_vm::vm::security::verify_secure_runner;
use cairo_vm::vm::vm_core::VirtualMachine;
use cairo_vm::{any_box, Felt252};
use tonic::{Request, Response, Status};

use madara_prover_common::models::{Proof, ProverConfig, ProverWorkingDirectory};
use stone_prover::error::ProverError;

use crate::cairo::{extract_execution_artifacts, ExecutionArtifacts, ExecutionError};
use crate::services::common::{
    call_prover, format_prover_error, get_prover_parameters, verify_and_annotate_proof,
};
use crate::services::starknet_prover::starknet_prover_proto::starknet_prover_server::StarknetProver;
use crate::services::starknet_prover::starknet_prover_proto::{
    StarknetExecutionRequest, StarknetProverResponse,
};

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
) -> Result<(CairoRunner, VirtualMachine), CairoRunError> {
    let secure_run = cairo_run_config
        .secure_run
        .unwrap_or(!cairo_run_config.proof_mode);

    let mut cairo_runner = CairoRunner::new(
        program,
        cairo_run_config.layout,
        cairo_run_config.proof_mode,
    )?;
    for (key, value) in variables {
        cairo_runner.exec_scopes.insert_box(&key, value);
    }

    let mut vm = VirtualMachine::new(cairo_run_config.trace_enabled);
    let end = cairo_runner.initialize(&mut vm)?;
    // check step calculation

    cairo_runner
        .run_until_pc(end, &mut vm, hint_executor)
        .map_err(|err| VmException::from_vm_error(&cairo_runner, &vm, err))?;
    cairo_runner.end_run(
        cairo_run_config.disable_trace_padding,
        false,
        &mut vm,
        hint_executor,
    )?;

    vm.verify_auto_deductions()?;
    cairo_runner.read_return_values(&mut vm)?;
    if cairo_run_config.proof_mode {
        cairo_runner.finalize_segments(&mut vm)?;
    }
    if secure_run {
        verify_secure_runner(&cairo_runner, true, None, &mut vm)?;
    }
    cairo_runner.relocate(&mut vm, cairo_run_config.relocate_mem)?;

    Ok((cairo_runner, vm))
}

#[derive(thiserror::Error, Debug)]
enum BootloaderTaskError {
    #[error("Failed to read program: {0}")]
    Program(#[from] ProgramError),

    #[error("Failed to read PIE: {0}")]
    Pie(#[from] CairoPieError),
}

fn make_bootloader_tasks(
    programs: &[Vec<u8>],
    pies: &[Vec<u8>],
) -> Result<Vec<TaskSpec>, BootloaderTaskError> {
    let program_tasks = programs.iter().map(|program_bytes| {
        let program = Program::from_bytes(program_bytes, Some("main"));
        program
            .map(|program| TaskSpec {
                task: Task::Program(program),
            })
            .map_err(BootloaderTaskError::Program)
    });

    let cairo_pie_tasks = pies.iter().map(|pie_bytes| {
        let pie = CairoPie::from_bytes(pie_bytes);
        pie.map(|pie| TaskSpec {
            task: Task::Pie(pie),
        })
        .map_err(BootloaderTaskError::Pie)
    });

    program_tasks.chain(cairo_pie_tasks).collect()
}

pub fn run_bootloader_in_proof_mode(
    bootloader: &Program,
    tasks: Vec<TaskSpec>,
) -> Result<ExecutionArtifacts, ExecutionError> {
    let proof_mode = true;
    let layout = "starknet";

    let cairo_run_config = CairoRunConfig {
        entrypoint: "main",
        trace_enabled: true,
        relocate_mem: true,
        layout,
        proof_mode,
        secure_run: None,
        disable_trace_padding: false,
    };

    let n_tasks = tasks.len();

    let bootloader_input = BootloaderInput {
        simple_bootloader_input: SimpleBootloaderInput {
            fact_topologies_path: None,
            single_page: false,
            tasks,
        },
        bootloader_config: BootloaderConfig {
            simple_bootloader_program_hash: Felt252::from(0),
            supported_cairo_verifier_program_hashes: vec![],
        },
        packed_outputs: vec![PackedOutput::Plain(vec![]); n_tasks],
    };

    let mut hint_processor = BuiltinHintProcessor::new_empty();
    let variables = HashMap::<String, Box<dyn Any>>::from([
        ("bootloader_input".to_string(), any_box!(bootloader_input)),
        (
            "bootloader_program".to_string(),
            any_box!(bootloader.clone()),
        ),
    ]);

    let (cairo_runner, vm) = cairo_run(
        bootloader,
        &cairo_run_config,
        &mut hint_processor,
        variables,
    )?;

    extract_execution_artifacts(cairo_runner, vm)
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

        let bootloader_tasks = make_bootloader_tasks(&programs, &pies).map_err(|e| {
            Status::invalid_argument(format!("Could not parse programs/PIEs: {}", e))
        })?;

        let execution_artifacts =
            run_bootloader_in_proof_mode(&bootloader_program, bootloader_tasks)
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
