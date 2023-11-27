#[cfg(test)]
mod tests {
    use madara_prover_rpc_server::cairo::run_in_proof_mode;
    use test_cases::load_test_case_file;

    #[test]
    fn test_run_bootloader() {
        let bootloader_program = load_test_case_file("bootloader/bootloader_compiled.json");
        let result = run_in_proof_mode(bootloader_program.as_bytes()).unwrap();
    }
}
