use anyhow::{Context, Result};
use chrono;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "8080")]
    port: u16,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProofRequest {
    age: u32,
    bmi_multiplied: u32, // BMI * 10 to avoid decimals
}

#[derive(Debug, Serialize, Deserialize)]
struct ProofResponse {
    proof_hex: String,
    public_inputs: String,
    success: bool,
    message: String,
}

struct NoirProver {
    circuit_path: String,
}

impl NoirProver {
    fn new() -> Self {
        // Check if we're running in Docker (where circuit is at /app/noir-circuit)
        // or locally (where circuit is at ../noir-circuit)
        let circuit_path = if std::path::Path::new("/app/noir-circuit").exists() {
            "/app/noir-circuit".to_string()
        } else {
            "../noir-circuit".to_string()
        };
        
        Self {
            circuit_path,
        }
    }

    async fn generate_proof(&self, request: ProofRequest) -> Result<ProofResponse> {
        let circuit_path = Path::new(&self.circuit_path);

        // Step 1: Write private inputs to Prover.toml
        let prover_toml_content = format!(
            r#"age = "{}"
bmi = "{}"
min_age = "10"
max_age = "25"
min_bmi = "185"
max_bmi = "249""#,
            request.age, request.bmi_multiplied
        );

        let prover_path = circuit_path.join("Prover.toml");
        fs::write(&prover_path, prover_toml_content)?;

        // Step 2: Execute to generate witness (this will create target/insurance_verifier.gz)
        let execute_output = Command::new("nargo")
            .arg("execute")
            .current_dir(&circuit_path)
            .output()
            .context("Failed to execute circuit")?;

        if !execute_output.status.success() {
            return Ok(ProofResponse {
                proof_hex: String::new(),
                public_inputs: String::new(),
                success: false,
                message: format!(
                    "Circuit execution failed. The inputs don't satisfy the constraints: {}",
                    String::from_utf8_lossy(&execute_output.stderr)
                ),
            });
        }

        // Check if witness file was generated (insurance_verifier.gz)
        let witness_gz_path = circuit_path.join("target").join("insurance_verifier.gz");
        let witness_path = circuit_path.join("target").join("insurance_verifier");
        if !witness_gz_path.exists() && !witness_path.exists() {
            return Ok(ProofResponse {
                proof_hex: String::new(),
                public_inputs: String::new(),
                success: false,
                message: "Witness file was not generated after circuit execution".to_string(),
            });
        }

        // Step 3: Generate proof using bb (Barretenberg) with correct command
        let prove_output = Command::new("bb")
            .args(&[
                "prove",
                "-b", "./target/insurance_verifier.json",
                "-w", "./target/insurance_verifier",
                "-o", "./target",
                "--oracle_hash", "keccak",
                "--output_format", "bytes_and_fields"
            ])
            .current_dir(&circuit_path)
            .output()
            .context("Failed to generate proof with bb")?;

        if !prove_output.status.success() {
            return Ok(ProofResponse {
                proof_hex: String::new(),
                public_inputs: String::new(),
                success: false,
                message: format!(
                    "Proof generation failed: {}",
                    String::from_utf8_lossy(&prove_output.stderr)
                ),
            });
        }

        // Debug: Check what files were actually created
        let target_dir = circuit_path.join("target");
        let proof_path = target_dir.join("proof");
        let public_inputs_path = target_dir.join("public_inputs");
        
        // Step 4: Convert proof to hex format using the specified method
        if !proof_path.exists() {
            return Ok(ProofResponse {
                proof_hex: String::new(),
                public_inputs: String::new(),
                success: false,
                message: format!("Proof file was not generated at path: {}", proof_path.display()),
            });
        }

        let hex_conversion_output = Command::new("sh")
            .arg("-c")
            .arg(format!("echo -n '0x'; cat '{}' | od -An -v -t x1 | tr -d ' \n'", proof_path.display()))
            .output()
            .context("Failed to convert proof to hex format")?;

        if !hex_conversion_output.status.success() {
            return Ok(ProofResponse {
                proof_hex: String::new(),
                public_inputs: String::new(),
                success: false,
                message: format!(
                    "Failed to convert proof to hex: {}",
                    String::from_utf8_lossy(&hex_conversion_output.stderr)
                ),
            });
        }

        let proof_hex = String::from_utf8_lossy(&hex_conversion_output.stdout).trim().to_string();

        // Step 5: Read public inputs from the correct location
        // First try to read the formatted JSON version
        let public_inputs_fields_path = circuit_path.join("target").join("public_inputs_fields.json");
        let public_inputs_path = circuit_path.join("target").join("public_inputs");
        
        let public_inputs = if public_inputs_fields_path.exists() {
            // Prefer the JSON formatted version
            match fs::read_to_string(&public_inputs_fields_path) {
                Ok(content) => content.trim().to_string(),
                Err(e) => {
                    return Ok(ProofResponse {
                        proof_hex,
                        public_inputs: String::new(),
                        success: false,
                        message: format!("Failed to read public inputs fields JSON at {}: {}", public_inputs_fields_path.display(), e),
                    });
                }
            }
        } else if public_inputs_path.exists() {
            // Fallback to raw public_inputs and format it properly
            match fs::read_to_string(&public_inputs_path) {
                Ok(text) => {
                    text.trim().to_string()
                },
                Err(_) => {
                    // If reading as text fails, read as binary and format as individual field elements
                    match fs::read(&public_inputs_path) {
                        Ok(bytes) => {
                            // Each field element is 32 bytes (64 hex characters)
                            let hex_string = hex::encode(bytes);
                            if hex_string.len() % 64 == 0 && !hex_string.is_empty() {
                                let mut field_elements = Vec::new();
                                for i in (0..hex_string.len()).step_by(64) {
                                    let end = std::cmp::min(i + 64, hex_string.len());
                                    field_elements.push(format!("\"0x{}\"", &hex_string[i..end]));
                                }
                                format!("[{}]", field_elements.join(","))
                            } else {
                                format!("0x{}", hex_string)
                            }
                        },
                        Err(e) => {
                            return Ok(ProofResponse {
                                proof_hex,
                                public_inputs: String::new(),
                                success: false,
                                message: format!("Failed to read public inputs file at {}: {}", public_inputs_path.display(), e),
                            });
                        }
                    }
                }
            }
        } else {
            return Ok(ProofResponse {
                proof_hex,
                public_inputs: String::new(),
                success: false,
                message: format!("Neither public_inputs_fields.json nor public_inputs file was generated at {}", circuit_path.join("target").display()),
            });
        };

        Ok(ProofResponse {
            proof_hex,
            public_inputs,
            success: true,
            message: "Proof generated successfully! The user is eligible for insurance discount.".to_string(),
        })
    }
}

async fn handle_client(mut stream: TcpStream) -> Result<()> {
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    let prover = NoirProver::new();

    // Send welcome message
    writer.write_all(b"ZK Insurance Verifier Server\n").await?;
    writer.write_all(b"============================\n").await?;
    writer.write_all(b"Enter age (10-25): ").await?;
    writer.flush().await?;

    // Read age
    line.clear();
    reader.read_line(&mut line).await?;
    let age: u32 = line.trim().parse().context("Invalid age input")?;

    // Ask for BMI
    writer.write_all(b"Enter BMI multiplied by 10 (185-249): ").await?;
    writer.flush().await?;

    // Read BMI
    line.clear();
    reader.read_line(&mut line).await?;
    let bmi_multiplied: u32 = line.trim().parse().context("Invalid BMI input")?;

    let request = ProofRequest {
        age,
        bmi_multiplied,
    };

    writer.write_all(b"\nGenerating proof...\n").await?;
    writer.write_all(b"Step 1: Writing inputs to Prover.toml...\n").await?;
    writer.write_all(b"Step 2: Executing circuit to generate witness (nargo execute)...\n").await?;
    writer.write_all(b"Step 3: Generating proof with Barretenberg (bb prove)...\n").await?;
    writer.write_all(b"Step 4: Converting proof to hex format...\n").await?;
    writer.flush().await?;

    match prover.generate_proof(request).await {
        Ok(response) => {
            let response_text = format!(
                "\n=== PROOF GENERATION RESULT ===\nSuccess: {}\nMessage: {}\n",
                response.success, response.message
            );
            writer.write_all(response_text.as_bytes()).await?;

            if response.success {
                // Display proof in hex format
                writer.write_all(b"\n=== PROOF (HEX FORMAT) ===\n").await?;
                writer.write_all(response.proof_hex.as_bytes()).await?;
                writer.write_all(b"\n").await?;

                // Display public inputs
                writer.write_all(b"\n=== PUBLIC INPUTS ===\n").await?;
                writer.write_all(response.public_inputs.as_bytes()).await?;
                writer.write_all(b"\n").await?;

                // Save proof and public inputs to files with timestamp
                let timestamp = chrono::Utc::now().timestamp();
                let proof_filename = format!("proof_{}.hex", timestamp);
                let public_inputs_filename = format!("public_inputs_{}.txt", timestamp);
                
                fs::write(&proof_filename, &response.proof_hex)?;
                fs::write(&public_inputs_filename, &response.public_inputs)?;
                
                let save_msg = format!(
                    "\nFiles saved:\n  - Proof: {}\n  - Public Inputs: {}\n",
                    proof_filename, public_inputs_filename
                );
                writer.write_all(save_msg.as_bytes()).await?;

                // Provide verification command hint
                writer.write_all(b"\n=== VERIFICATION ===\n").await?;
                writer.write_all(b"To verify this proof, use the proof hex and public inputs displayed above.\n").await?;
                writer.write_all(b"The proof has been generated using the correct bb command format.\n").await?;
            } else {
                // Display the error message for failed proof generation
                writer.write_all(b"\n=== ERROR DETAILS ===\n").await?;
                writer.write_all(response.message.as_bytes()).await?;
                writer.write_all(b"\n").await?;
            }
        }
        Err(e) => {
            let error_msg = format!("Error generating proof: {}\n", e);
            writer.write_all(error_msg.as_bytes()).await?;
        }
    }

    writer.write_all(b"\nConnection will close. Thanks for using ZK Insurance Verifier!\n").await?;
    writer.flush().await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let addr = format!("0.0.0.0:{}", args.port);
    
    println!("ZK Insurance Verifier TCP Server");
    println!("================================");
    println!("Listening on {}", addr);
    println!("Connect using: nc 127.0.0.1 {}", args.port);
    println!("Or: telnet 127.0.0.1 {}", args.port);
    println!();
    println!("Note: Make sure 'bb' (Barretenberg) and 'nargo' are installed and in PATH");
    println!("Requirements:");
    println!("  - Valid age range: 10-25");
    println!("  - Valid BMI range: 18.5-24.9 (multiplied by 10: 185-249)");
    println!();

    let listener = TcpListener::bind(&addr).await?;

    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                println!("New connection from: {}", addr);
                
                tokio::spawn(async move {
                    if let Err(e) = handle_client(stream).await {
                        eprintln!("Error handling client {}: {}", addr, e);
                    } else {
                        println!("Client {} disconnected", addr);
                    }
                });
            }
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
            }
        }
    }
}