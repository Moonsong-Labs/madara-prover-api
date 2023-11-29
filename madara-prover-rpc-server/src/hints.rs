use std::collections::HashMap;
use std::rc::Rc;

use cairo_vm::felt::Felt252;
use cairo_vm::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::{
    BuiltinHintProcessor, HintFunc,
};
use cairo_vm::hint_processor::hint_processor_definition::HintReference;
use cairo_vm::serde::deserialize_program::ApTracking;
use cairo_vm::types::exec_scope::ExecutionScopes;
use cairo_vm::vm::errors::hint_errors::HintError;
use cairo_vm::vm::vm_core::VirtualMachine;

const PREPARE_SIMPLE_BOOTLOADER_OUTPUT_SEGMENT: &str =
    "from starkware.cairo.bootloaders.bootloader.objects import BootloaderInput
bootloader_input = BootloaderInput.Schema().load(program_input)

ids.simple_bootloader_output_start = segments.add()

# Change output builtin state to a different segment in preparation for calling the
# simple bootloader.
output_builtin_state = output_builtin.get_state()
output_builtin.new_state(base=ids.simple_bootloader_output_start)";

const PREPARE_SIMPLE_BOOTLOADER_INPUT: &str = "simple_bootloader_input = bootloader_input";

const RESTORE_BOOTLOADER_OUTPUT: &str = "# Restore the bootloader's output builtin state.
output_builtin.set_state(output_builtin_state)";

const LOAD_BOOTLOADER_CONFIG: &str =
    "from starkware.cairo.bootloaders.bootloader.objects import BootloaderConfig
bootloader_config: BootloaderConfig = bootloader_input.bootloader_config

ids.bootloader_config = segments.gen_arg(
    [
        bootloader_config.simple_bootloader_program_hash,
        len(bootloader_config.supported_cairo_verifier_program_hashes),
        bootloader_config.supported_cairo_verifier_program_hashes,
    ],
)";

const SAVE_OUTPUT_POINTER: &str = "output_start = ids.output_ptr";

const SAVE_PACKED_OUTPUTS: &str = "packed_outputs = bootloader_input.packed_outputs";

const COMPUTE_FACT_TOPOLOGIES: &str = "from typing import List

from starkware.cairo.bootloaders.bootloader.utils import compute_fact_topologies
from starkware.cairo.bootloaders.fact_topology import FactTopology
from starkware.cairo.bootloaders.simple_bootloader.utils import (
    configure_fact_topologies,
    write_to_fact_topologies_file,
)

# Compute the fact topologies of the plain packed outputs based on packed_outputs and
# fact_topologies of the inner tasks.
plain_fact_topologies: List[FactTopology] = compute_fact_topologies(
    packed_outputs=packed_outputs, fact_topologies=fact_topologies,
)

# Configure the memory pages in the output builtin, based on plain_fact_topologies.
configure_fact_topologies(
    fact_topologies=plain_fact_topologies, output_start=output_start,
    output_builtin=output_builtin,
)

# Dump fact topologies to a json file.
if bootloader_input.fact_topologies_path is not None:
    write_to_fact_topologies_file(
        fact_topologies_path=bootloader_input.fact_topologies_path,
        fact_topologies=plain_fact_topologies,
    )";

const ENTER_PACKED_OUTPUT_SCOPE: &str =
    "from starkware.cairo.bootloaders.bootloader.objects import PackedOutput

task_id = len(packed_outputs) - ids.n_subtasks
packed_output: PackedOutput = packed_outputs[task_id]

vm_enter_scope(new_scope_locals=dict(packed_output=packed_output))";

const IMPORT_PACKED_OUTPUT_SCHEMAS: &str =
    "from starkware.cairo.bootloaders.bootloader.objects import (
    CompositePackedOutput,
    PlainPackedOutput,
)";

const IS_PLAIN_PACKED_OUTPUT: &str = "isinstance(packed_output, PlainPackedOutput)";
const ASSERT_IS_COMPOSITE_PACKED_OUTPUT: &str =
    "assert isinstance(packed_output, CompositePackedOutput)";

const GUESS_PRE_IMAGE_OF_SUBTASKS_OUTPUT_HASH: &str = "data = packed_output.elements_for_hash()
ids.nested_subtasks_output_len = len(data)
ids.nested_subtasks_output = segments.gen_arg(data)";

const SET_PACKED_OUTPUT_TO_SUBTASKS: &str = "packed_outputs = packed_output.subtasks";

fn unimplemented_hint(
    _vm: &mut VirtualMachine,
    _exec_scopes: &mut ExecutionScopes,
    _ids_data: &HashMap<String, HintReference>,
    _ap_tracking: &ApTracking,
    _constants: &HashMap<String, Felt252>,
) -> Result<(), HintError> {
    Ok(())
}

/*
Implements hint:
%{
    output_start = ids.output_ptr
%}
*/
fn save_output_pointer_hint(
    _vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    _ap_tracking: &ApTracking,
    _constants: &HashMap<String, Felt252>,
) -> Result<(), HintError> {
    let output_ptr = ids_data.get("output_ptr")
        .ok_or(HintError::UnknownIdentifier("output_ptr".to_owned().into_boxed_str()))?
        .clone();
    exec_scopes.insert_value("output_start", output_ptr);
    Ok(())
}

/*
Implements hint:
%{
    packed_outputs = bootloader_input.packed_outputs
%}
*/
fn save_packed_outputs_hint(
    _vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    _ids_data: &HashMap<String, HintReference>,
    _ap_tracking: &ApTracking,
    _constants: &HashMap<String, Felt252>,
) -> Result<(), HintError> {
    let bootloader_input = exec_scopes.get("bootloader_input")?;
    let packed_outputs = bootloader_input; // TODO: need type for bootloader_input / query its packed_outputs field
    exec_scopes.insert_value("packed_outputs", packed_outputs);
    Ok(())
}

/*
Implements hint:
%{
    packed_outputs = packed_output.subtasks
%}
*/
fn set_packed_output_to_subtasks_hint(
    _vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    _ids_data: &HashMap<String, HintReference>,
    _ap_tracking: &ApTracking,
    _constants: &HashMap<String, Felt252>,
) -> Result<(), HintError> {
    let packed_outputs = exec_scopes.get("packed_outputs")?;
    let subtasks = packed_outputs; // TODO: need type for packed_output / query its subtasks field
    exec_scopes.insert_value("packed_outputs", subtasks);
    Ok(())
}

pub fn hint_processor() -> BuiltinHintProcessor {
    let mut hint_processor = BuiltinHintProcessor::new_empty();

    let unimplemented_hint = Rc::new(HintFunc(Box::new(unimplemented_hint)));

    hint_processor.add_hint(
        PREPARE_SIMPLE_BOOTLOADER_OUTPUT_SEGMENT.to_string(),
        unimplemented_hint.clone(),
    );
    hint_processor.add_hint(
        PREPARE_SIMPLE_BOOTLOADER_INPUT.to_string(),
        unimplemented_hint.clone(),
    );
    hint_processor.add_hint(
        RESTORE_BOOTLOADER_OUTPUT.to_string(),
        unimplemented_hint.clone(),
    );
    hint_processor.add_hint(
        LOAD_BOOTLOADER_CONFIG.to_string(),
        unimplemented_hint.clone(),
    );
    hint_processor.add_hint(
        SAVE_OUTPUT_POINTER.to_string(),
        Rc::new(HintFunc(Box::new(save_output_pointer_hint)))
    );
    hint_processor.add_hint(
        SAVE_PACKED_OUTPUTS.to_string(),
        Rc::new(HintFunc(Box::new(save_packed_outputs_hint)))
    );
    hint_processor.add_hint(
        COMPUTE_FACT_TOPOLOGIES.to_string(),
        unimplemented_hint.clone(),
    );
    hint_processor.add_hint(
        ENTER_PACKED_OUTPUT_SCOPE.to_string(),
        unimplemented_hint.clone(),
    );
    hint_processor.add_hint(
        IMPORT_PACKED_OUTPUT_SCHEMAS.to_string(),
        unimplemented_hint.clone(),
    );
    hint_processor.add_hint(
        IS_PLAIN_PACKED_OUTPUT.to_string(),
        unimplemented_hint.clone(),
    );
    hint_processor.add_hint(
        ASSERT_IS_COMPOSITE_PACKED_OUTPUT.to_string(),
        unimplemented_hint.clone(),
    );
    hint_processor.add_hint(
        GUESS_PRE_IMAGE_OF_SUBTASKS_OUTPUT_HASH.to_string(),
        unimplemented_hint.clone(),
    );
    hint_processor.add_hint(
        SET_PACKED_OUTPUT_TO_SUBTASKS.to_string(),
        Rc::new(HintFunc(Box::new(set_packed_output_to_subtasks_hint)))

    );

    hint_processor
}
