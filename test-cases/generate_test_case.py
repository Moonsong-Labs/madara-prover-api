import argparse
import json
import math
from dataclasses import dataclass
from pathlib import Path
import shutil
import subprocess
from typing import Optional, Tuple, List
import re


def compile_cairo_program(program_path: Path, output_path: Path):
    """
    Compiles a Cairo program.

    :param program_path: Path to the program to compile.
    :param output_path: Path to the compiled file.
    """

    subprocess.run(
        ["cairo-compile", program_path, "--output", output_path, "--proof_mode"],
        check=True,
        capture_output=True,
    )


@dataclass
class ExecutionArtifacts:
    public_input: Path
    private_input: Path
    memory: Path
    trace: Path
    nb_steps: int


def get_nb_steps(run_stdout: str) -> int:
    """
    Extracts the number of steps from the output of `cairo-run`.

    :param run_stdout: Output of `cairo-run`. Note that `--print_info` must be specified
                       when running `cairo-run`.
    :return: The number of Cairo steps of the program.
    """
    for line in run_stdout.splitlines():
        if m := re.match(r"Number of steps: (\d+).*", line):
            return int(m.group(1))

    raise ValueError(
        "Could not find number of steps in the run output. Is --print_info set?"
    )


def run_cairo_program(
    compiled_program_path: Path,
    output_dir: Path,
    program_name: str,
    program_input: Optional[Path] = None,
) -> ExecutionArtifacts:
    """
    Runs a Cairo program in proof mode and returns the run artifacts.

    :param compiled_program_path: Path to the compiled program file.
    :param output_dir: Output directory. Execution artifact files will be placed there.
    :param program_name: Name of the program. Used as the base name for generated files.
    :param program_input: Program input file, if any.
    :return: All execution artifacts.
    """

    public_input_file = output_dir / f"{program_name}_public_input.json"
    private_input_file = output_dir / f"{program_name}_private_input.json"
    memory_file = output_dir / f"{program_name}_memory.bin"
    trace_file = output_dir / f"{program_name}_trace.bin"

    command = [
        "cairo-run",
        f"--program={compiled_program_path}",
        "--layout=starknet_with_keccak",
        f"--air_public_input={public_input_file}",
        f"--air_private_input={private_input_file}",
        f"--trace_file={trace_file}",
        f"--memory_file={memory_file}",
        "--print_output",
        "--proof_mode",
        "--print_info",
    ]

    if program_input:
        command += [f"--program_input={program_input}"]

    result = subprocess.run(
        command,
        check=False,
        capture_output=True,
    )

    if result.returncode:
        print(f"Run failed: {result.stderr.decode('utf8')}")
        result.check_returncode()

    nb_steps = get_nb_steps(result.stdout.decode("utf-8"))

    return ExecutionArtifacts(
        public_input=public_input_file,
        private_input=private_input_file,
        memory=memory_file,
        trace=trace_file,
        nb_steps=nb_steps,
    )


def compute_fri_step_list(nb_steps: int, last_layer_degree_bound: int) -> List[int]:
    """
    Computes the FRI steps list based on the number of Cairo steps of the program.

    This computation is based on the documentation of the Stone prover:
    # log₂(#steps) + 4 = log₂(last_layer_degree_bound) + ∑fri_step_list
    # log₂(#steps) = log₂(last_layer_degree_bound) + ∑fri_step_list - 4
    # ∑fri_step_list = log₂(#steps) + 4 - log₂(last_layer_degree_bound)

    :param nb_steps: Number of Cairo steps of the program.
    :param last_layer_degree_bound: Last layer degree bound.
    :return: The FRI steps list.
    """

    program_n_steps_log = math.ceil(math.log(nb_steps, 2))
    last_layer_degree_bound_log = math.ceil(math.log(last_layer_degree_bound, 2))
    sigma_fri_step_list = program_n_steps_log + 4 - last_layer_degree_bound_log

    (q, r) = divmod(sigma_fri_step_list, 4)
    fri_step_list = [4] * q
    if r > 0:
        fri_step_list.append(r)

    return fri_step_list


def generate_prover_config(
    artifacts: ExecutionArtifacts, output_dir: Path
) -> Tuple[Path, Path]:
    """
    Generates the prover config and parameters files.

    :param artifacts: Execution artifacts.
    :param output_dir: Output directory. The generated files will be placed there.
    :return: The prover config and parameter files paths.
    """

    config_file = output_dir / "cpu_air_prover_config.json"
    parameter_file = output_dir / "cpu_air_params.json"

    config = {
        "cached_lde_config": {"store_full_lde": False, "use_fft_for_eval": False},
        "constraint_polynomial_task_size": 256,
        "n_out_of_memory_merkle_layers": 1,
        "table_prover_n_tasks_per_segment": 32,
    }

    last_layer_degree_bound = 64
    fri_step_list = compute_fri_step_list(artifacts.nb_steps, last_layer_degree_bound)
    parameters = {
        "field": "PrimeField0",
        "stark": {
            "fri": {
                "fri_step_list": fri_step_list,
                "last_layer_degree_bound": last_layer_degree_bound,
                "n_queries": 18,
                "proof_of_work_bits": 24,
            },
            "log_n_cosets": 4,
        },
        "use_extension_field": False,
    }

    with config_file.open("w") as f:
        json.dump(config, f, indent=2)

    with parameter_file.open("w") as f:
        json.dump(parameters, f, indent=2)

    return config_file, parameter_file


def prove_cairo_program(
    artifacts: ExecutionArtifacts,
    prover_config: Path,
    prover_parameters: Path,
    output_proof_path: Path,
):
    """
    Proves a Cairo program execution.

    :param artifacts: Execution artifacts.
    :param prover_config: Path to the prover configuration file.
    :param prover_parameters: Path to the prover parameters file.
    :param output_proof_path: Where to store the generated proof file.
    """

    result = subprocess.run(
        [
            "cpu_air_prover",
            f"--out_file={output_proof_path}",
            f"--private_input_file={artifacts.private_input}",
            f"--public_input_file={artifacts.public_input}",
            f"--prover_config_file={prover_config}",
            f"--parameter_file={prover_parameters}",
        ],
        check=False,
        capture_output=True,
    )

    if result.returncode:
        print(f"Could not prove program: {result.stderr}")
        result.check_returncode()


def main(args: argparse.Namespace):
    program_path = Path(args.program)
    if not program_path.is_file():
        raise RuntimeError(f"Program {program_path} is not a file")

    program_input_path = Path(args.program_input) if args.program_input else None
    if program_input_path and not program_input_path.is_file():
        raise RuntimeError(f"Program input file {program_input_path} is not a file")

    program_name = program_path.stem

    # Create the output directory if it does not exist yet
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    # Copy the program in the output directory
    try:
        shutil.copy2(program_path, output_dir)
    except shutil.SameFileError:
        pass

    # Compile the program
    compiled_program_path = output_dir / f"{program_name}_compiled.json"
    compile_cairo_program(program_path, compiled_program_path)

    # Run the program
    artifacts = run_cairo_program(
        compiled_program_path=compiled_program_path,
        output_dir=output_dir,
        program_name=program_name,
    )

    # Generate the prover config and parameters
    prover_config, prover_params = generate_prover_config(artifacts, output_dir)

    # Prove the program execution
    proof_file = output_dir / f"{program_name}_proof.json"
    prove_cairo_program(
        artifacts=artifacts,
        prover_config=prover_config,
        prover_parameters=prover_params,
        output_proof_path=proof_file,
    )

    print(f"Proof successfully generated: {proof_file}")


def parse_args():
    parser = argparse.ArgumentParser(description="Compiles, runs and prove any Cairo v0 program.")
    parser.add_argument("program", help="Cairo v0 program.")
    parser.add_argument("--program-input", "-i", help="Program input file.", required=False)
    parser.add_argument("--output-dir", "-o", help="Output directory.")
    return parser.parse_args()


if __name__ == "__main__":
    main(parse_args())
