# Test cases

This crate contains various Cairo programs and their execution artifacts.
These files can be used for unit and integration tests.

## Generate a new test case

The `generate_test_case.py` script can compile, run and prove any Cairo v0 program passed as input.
To use it, you must first set up a Python virtual environment and install the requirements.

```shell
python -m virtualenv venv
source venv/bin/activate
pip install -r requirements.txt
```

> Note that the cairo-lang package is not compatible with Python>=3.11.

The script also requires you to have the Stone prover (`cpu_air_prover`) in your PATH.
You can either follow the instructions on the [Stone prover repository](https://github.com/starkware-libs/stone-prover)
and put the binaries somewhere on your system (ex: `/opt/stone-prover/bin`)
or build the workspace and add `target/debug` to your PATH.

Then, if you have a `program.cairo` file on your system, you can generate the whole test case by running the following command:
```shell
python test-cases/generate_test_case \
  program.cairo \
  [--program-input program-input.json] \
  --output-dir test-cases/cases/program
```
