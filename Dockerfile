FROM rust:1.89 AS builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    curl \
    git \
    build-essential \
    pkg-config \
    libssl-dev \
    jq \
    && rm -rf /var/lib/apt/lists/*

# Install Noir
RUN curl -L https://raw.githubusercontent.com/noir-lang/noirup/main/install | bash
ENV PATH="/root/.nargo/bin:${PATH}"
RUN noirup

# Install Barretenberg with specific version
RUN curl -L https://raw.githubusercontent.com/AztecProtocol/aztec-packages/refs/heads/master/barretenberg/bbup/install | bash
ENV PATH="/root/.bb:${PATH}"
# Install a specific known working version of barretenberg
RUN /root/.bb/bbup -v 0.87.0 && \
    ln -sf /root/.bb/bb /usr/local/bin/bb

# Verify installations
RUN nargo --version
RUN bb --version || echo "Barretenberg verification skipped"

# Copy circuit files
WORKDIR /app
COPY noir-circuit ./noir-circuit

# Build the circuit
WORKDIR /app/noir-circuit
RUN nargo compile

# Copy and build server
WORKDIR /app
COPY server ./server
WORKDIR /app/server
RUN cargo build --release

# Runtime stage - Use specific Ubuntu version for better compatibility
FROM ubuntu:24.04

RUN apt-get update && apt-get install -y \
    libssl-dev \
    ca-certificates \
    curl \
    git \
    build-essential \
    cmake \
    jq \
    libc++1 \
    libc++abi1 \
    && rm -rf /var/lib/apt/lists/*

# Install Noir in runtime
RUN curl -L https://raw.githubusercontent.com/noir-lang/noirup/main/install | bash
ENV PATH="/root/.nargo/bin:${PATH}"
RUN noirup

# Install Barretenberg in runtime with specific version
RUN curl -L https://raw.githubusercontent.com/AztecProtocol/aztec-packages/refs/heads/master/barretenberg/bbup/install | bash
ENV PATH="/root/.bb:${PATH}"
RUN /root/.bb/bbup -v 0.87.0 && \
    ln -sf /root/.bb/bb /usr/local/bin/bb

# Verify installations
RUN nargo --version
RUN bb --version || echo "Barretenberg verification skipped"

WORKDIR /app

# Copy circuit and compiled server
COPY --from=builder /app/noir-circuit ./noir-circuit
COPY --from=builder /app/server/target/release/zk-insurance-server ./

EXPOSE 8080

CMD ["./zk-insurance-server"]
