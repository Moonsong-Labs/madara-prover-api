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

const PREPARE_SIMPLE_BOOTLOADER_OUTPUT_SEGMENT: &str = "from starkware.cairo.bootloaders.bootloader.objects import BootloaderInput\nbootloader_input = BootloaderInput.Schema().load(program_input)\n\nids.simple_bootloader_output_start = segments.add()\n\n# Change output builtin state to a different segment in preparation for calling the\n# simple bootloader.\noutput_builtin_state = output_builtin.get_state()\noutput_builtin.new_state(base=ids.simple_bootloader_output_start)";

fn prepare_simple_bootloader_output_segment(
    _vm: &mut VirtualMachine,
    _exec_scopes: &mut ExecutionScopes,
    _ids_data: &HashMap<String, HintReference>,
    _ap_tracking: &ApTracking,
    _constants: &HashMap<String, Felt252>,
) -> Result<(), HintError> {
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
