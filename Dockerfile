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

# Detect architecture
RUN echo "Building for architecture: $(uname -m)"

# Install Noir with architecture detection
RUN curl -L https://raw.githubusercontent.com/noir-lang/noirup/main/install | bash
ENV PATH="/root/.nargo/bin:${PATH}"
RUN noirup

# Install Barretenberg with architecture handling
RUN curl -L https://raw.githubusercontent.com/AztecProtocol/aztec-packages/refs/heads/master/barretenberg/bbup/install | bash
ENV PATH="/root/.bb:${PATH}"

# Apply ARM64 fix to bbup script before running it
RUN ARCH=$(uname -m) && \
    echo "Installing Barretenberg for architecture: $ARCH" && \
    if [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then \
        echo "ARM64 detected. Applying fix to bbup script..." && \
        # Create a fixed version of bbup script for ARM64 \
        cp /root/.bb/bbup /root/.bb/bbup.orig && \
        awk ' \
        /local binary_url=/ { \
            print "    if [[ \"$architecture\" == \"arm64\" ]] && [[ \"$platform\" == \"linux\" ]] && [[ \"$release_tag\" == \"v0.87.0\" ]]; then"; \
            print "        release_tag=\"v0.87.2\""; \
            print "    fi"; \
            print $0; \
            next \
        } \
        { print } \
        ' /root/.bb/bbup.orig > /root/.bb/bbup && \
        chmod +x /root/.bb/bbup && \
        echo "bbup script modified for ARM64 compatibility"; \
    fi && \
    /root/.bb/bbup && \
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
    && rm -rf /var/lib/apt/lists/*

# Detect architecture in runtime
RUN echo "Runtime architecture: $(uname -m)"

# Install Noir in runtime
RUN curl -L https://raw.githubusercontent.com/noir-lang/noirup/main/install | bash
ENV PATH="/root/.nargo/bin:${PATH}"
RUN noirup

# Install Barretenberg in runtime with architecture handling
RUN curl -L https://raw.githubusercontent.com/AztecProtocol/aztec-packages/refs/heads/master/barretenberg/bbup/install | bash
ENV PATH="/root/.bb:${PATH}"

# Apply ARM64 fix to bbup script before running it
RUN ARCH=$(uname -m) && \
    echo "Installing Barretenberg for runtime architecture: $ARCH" && \
    if [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then \
        echo "ARM64 runtime detected. Applying fix to bbup script..." && \
        # Create a fixed version of bbup script for ARM64 \
        cp /root/.bb/bbup /root/.bb/bbup.orig && \
        awk ' \
        /local binary_url=/ { \
            print "    if [[ \"$architecture\" == \"arm64\" ]] && [[ \"$platform\" == \"linux\" ]] && [[ \"$release_tag\" == \"v0.87.0\" ]]; then"; \
            print "        release_tag=\"v0.87.2\""; \
            print "    fi"; \
            print $0; \
            next \
        } \
        { print } \
        ' /root/.bb/bbup.orig > /root/.bb/bbup && \
        chmod +x /root/.bb/bbup && \
        echo "bbup script modified for ARM64 compatibility"; \
    fi && \
    /root/.bb/bbup && \
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