# Process to generate and verify proof using Noir and Barretenberg:

1. Build circuit in `main.nr`.
2. Do `nargo compile`: This will compile your source code into a Noir build artifact to be stored in the ./target directory.
3. Do `nargo check`: This will generate a `Prover.toml` you can fill with the values you want to prove.
4. Add private and public inputs in `Prover.toml`.
5. `nargo execute <witness-name>`: execute the circuit with nargo, gives error if inputs do not satisfy the constrains.

## Verifying using contract:

6. `bb write_vk -b ./target/<noir_artifact_name>.json -o ./target --oracle_hash keccak`: Generate the verification key. You need to pass the `--oracle_hash keccak` flag when generating vkey and proving to instruct bb to use keccak as the hash function, which is more optimal in Solidity.
7. `bb prove -b ./target/<circuit-name>.json -w ./target/<witness-name> -o ./target --oracle_hash keccak --output_format bytes_and_fields`: use the proving backend to generate prove.
8. `echo -n "0x"; cat ./target/proof | od -An -v -t x1 | tr -d $' \n'`:Print the proof bytes as a hex string.
9. Use the proof hex string and public_inputs_fields.json to verify using these steps: https://noir-lang.org/docs/dev/how_to/how-to-solidity-verifier

## Verifying using Barretenberg:

6. `bb prove --scheme ultra_honk -b ./target/hello_world.json -w ./target/witness-name.gz -o ./target/proof`: Prove the valid execution of your program and generate proof inside `target/proof`.
7. `bb write_vk --scheme ultra_honk -b ./target/hello_world.json -o ./target/vk`: You can then compute the verification key for your Noir program inside `target/vk`.
8. `bb verify --scheme ultra_honk -k ./target/vk -p ./target/proof`: verify the proof.