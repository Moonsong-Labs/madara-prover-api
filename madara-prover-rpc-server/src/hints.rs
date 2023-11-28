use std::collections::HashMap;
use std::rc::Rc;

use cairo_vm::felt::Felt252;
use cairo_vm::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::{
    BuiltinHintProcessor, HintFunc,
};
use cairo_vm::hint_processor::builtin_hint_processor::hint_utils::insert_value_from_var_name;
use cairo_vm::hint_processor::hint_processor_definition::HintReference;
use cairo_vm::serde::deserialize_program::ApTracking;
use cairo_vm::types::exec_scope::ExecutionScopes;
use cairo_vm::vm::errors::hint_errors::HintError;
use cairo_vm::vm::errors::vm_errors::VirtualMachineError;
use cairo_vm::vm::runners::builtin_runner::{BuiltinRunner, OutputBuiltinRunner};
use cairo_vm::vm::vm_core::VirtualMachine;
use serde::Deserialize;

const PREPARE_SIMPLE_BOOTLOADER_OUTPUT_SEGMENT: &str =
    "from starkware.cairo.bootloaders.bootloader.objects import BootloaderInput
bootloader_input = BootloaderInput.Schema().load(program_input)

ids.simple_bootloader_output_start = segments.add()

# Change output builtin state to a different segment in preparation for calling the
# simple bootloader.
output_builtin_state = output_builtin.get_state()
output_builtin.new_state(base=ids.simple_bootloader_output_start)";

const PREPARE_SIMPLE_BOOTLOADER_INPUT: &str = "simple_bootloader_input = bootloader_input";

fn unimplemented_hint(
    _vm: &mut VirtualMachine,
    _exec_scopes: &mut ExecutionScopes,
    _ids_data: &HashMap<String, HintReference>,
    _ap_tracking: &ApTracking,
    _constants: &HashMap<String, Felt252>,
) -> Result<(), HintError> {
    Ok(())
}

#[derive(Deserialize, Debug)]
struct BootloaderInput {}

fn get_output_builtin(
    vm: &mut VirtualMachine,
) -> Result<&mut OutputBuiltinRunner, VirtualMachineError> {
    for builtin in vm.get_builtin_runners_as_mut() {
        if let BuiltinRunner::Output(output_builtin) = builtin {
            return Ok(output_builtin);
        };
    }

    // TODO: this is not the correct error, add an error type and helper in cairo-vm
    Err(VirtualMachineError::NoSignatureBuiltin)
}

/// Implements
/// %{
///     from starkware.cairo.bootloaders.bootloader.objects import BootloaderInput
///     bootloader_input = BootloaderInput.Schema().load(program_input)
///
///     ids.simple_bootloader_output_start = segments.add()
///
///     # Change output builtin state to a different segment in preparation for calling the
///     # simple bootloader.
///     output_builtin_state = output_builtin.get_state()
///     output_builtin.new_state(base=ids.simple_bootloader_output_start)
/// %}
fn prepare_simple_bootloader_output_segment(
    vm: &mut VirtualMachine,
    _exec_scopes: &mut ExecutionScopes,
    ids_data: &HashMap<String, HintReference>,
    ap_tracking: &ApTracking,
    _constants: &HashMap<String, Felt252>,
) -> Result<(), HintError> {
    // ids.simple_bootloader_output_start = segments.add()
    // let new_output_builtin = OutputBuiltinRunner::new(true).initialize_segments(&mut vm.seg);

    let new_segment_base = vm.add_memory_segment();
    insert_value_from_var_name(
        "simple_bootloader_output_start",
        new_segment_base,
        vm,
        ids_data,
        ap_tracking,
    )?;

    // output_builtin_state = output_builtin.get_state()
    // output_builtin.new_state(base=ids.simple_bootloader_output_start)
    let output_builtin = get_output_builtin(vm)?;
    println!("{:?}", output_builtin);
    // let new_output_builtin = OutputBuiltinRunner::n

    Ok(())
}

pub fn hint_processor() -> BuiltinHintProcessor {
    let mut hint_processor = BuiltinHintProcessor::new_empty();

    let prepare_simple_bootloader_output_segment_hint =
        HintFunc(Box::new(prepare_simple_bootloader_output_segment));
    hint_processor.add_hint(
        PREPARE_SIMPLE_BOOTLOADER_OUTPUT_SEGMENT.to_string(),
        Rc::new(prepare_simple_bootloader_output_segment_hint),
    );

    hint_processor
}
