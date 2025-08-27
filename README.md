# ZK Insurance Verifier

A zero-knowledge proof system for verifying insurance discount eligibility based on age and BMI without revealing the actual values. The Rust App generates a server which accepts private inputs and generates proof and public inputs.

The whole app runs inside the Marlin oyster's CVM, which secures the App inside TEE enclaves (Your private inputs are secured and can't be leaked).

## Features

- Proves age is between 10-25 years
- Proves BMI is between 18.5-24.9
- Generates zero-knowledge proofs without revealing actual values
- Docker containerized for easy deployment
- TCP server interface accessible via `nc` or `telnet`
- Concurrent client support

## Prerequisites

- Docker and Docker Compose
- Git

## Run Locally

1. Compile the circuit:
```bash
cd noir-circuit
nargo compile
```

2. Run the server:
```bash
cd server
cargo run
```

3. In a new terminal, connect to the server:
```bash
nc 127.0.0.1 8080
```

4. Follow the prompts to enter your age (10-25) and BMI multiplied by 10 (185-249).

## Usage Example

1. Build Docker Image and Publish on Docker Hub:
```bash
docker buildx build --platform linux/amd64,linux/arm64 -t ayushranjan123/insurance-verifier:latest --push .
```

2. Start the server locally:
```bash
sudo docker run --rm --init -p 8080:8080 ayushranjan123/insurance-verifier
```
Test a particular architecture (e.g ARM64):
```bash
docker pull --platform linux/arm64 ayushranjan123/insurance-verifier:latest
```
```bash
docker run --platform linux/arm64 --rm --init -p 8080:8080 ayushranjan123/insurance-verifier:latest
```

3. Connect from another terminal:
```bash
nc 127.0.0.1 8080
```
or
```bash
telnet 127.0.0.1 8080
```


4. Deploy On Oyster TEE:

For AMD64 Architecture-
```bash
oyster-cvm deploy --wallet-private-key <key> --duration-in-minutes 20 --docker-compose docker-compose.yml --arch amd64
```
For ARM64 Architecture-
```bash
oyster-cvm deploy --wallet-private-key <key> --duration-in-minutes 20 --docker-compose docker-compose.yml --instance-type c6g.xlarge
```

5. Start the server:
```bash
nc <IP> 8080
```
or
```bash
telnet <IP> 8080
```

6. Interaction example:
```
ZK Insurance Verifier Server
============================
Enter age (10-25): 20
Enter BMI multiplied by 10 (185-249): 220
Generating proof...

=== PROOF RESPONSE ===
Success: true
Message: Proof generated successfully! The user is eligible for insurance discount.
...
```
## Proof Verification

1. Deploy the `Verifier.sol` contract using Remix IDE, follow the steps from Noir Docs: https://noir-lang.org/docs/dev/how_to/how-to-solidity-verifier#step-2---compiling

2. Use proof and public inputs generated from the application to verify.

## Remote Attestation verification:

`oyster-cvm verify --enclave-ip <ip>`

## ARM64 Architecture Support

This project includes a workaround for ARM64 Linux compatibility issues with Barretenberg v0.87.0. The issue occurs because the ARM64 Linux binary for Barretenberg v0.87.0 is not published, causing proof generation to fail.

### The Problem
When running on ARM64 architecture (like Oyster ARM64 architecture enclaves), you might encounter:
```
=== PROOF GENERATION RESULT ===
Success: false
Message: Proof file was not generated at path: /app/noir-circuit/target/proof
```

### The Solution
Our Dockerfile automatically detects ARM64 architecture and applies a fix to the `bbup` script:

1. **Detection**: The build process detects if the architecture is `aarch64` or `arm64`
2. **Script Modification**: It modifies the `bbup` script to use Barretenberg v0.87.2 instead of v0.87.0 for ARM64 Linux
3. **Implementation**: The fix is applied using an `awk` script that inserts version override logic before the download URL construction

### Technical Details
The workaround adds this logic to the `bbup` script:
```bash
if [[ "$architecture" == "arm64" ]] && [[ "$platform" == "linux" ]] && [[ "$release_tag" == "v0.87.0" ]]; then
    release_tag="v0.87.2"
fi
```

This ensures that ARM64 Linux systems download the compatible v0.87.2 binary instead of the unavailable v0.87.0 binary.
