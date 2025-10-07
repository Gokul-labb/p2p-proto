# Security Documentation

This document outlines the security architecture, threat model, and security best practices for the P2P File Converter.

## Table of Contents

1. [Security Architecture](#security-architecture)
2. [Threat Model](#threat-model)
3. [Transport Security](#transport-security)
4. [Application Security](#application-security)
5. [Input Validation](#input-validation)
6. [Resource Protection](#resource-protection)
7. [Cryptographic Implementation](#cryptographic-implementation)
8. [Security Testing](#security-testing)
9. [Incident Response](#incident-response)
10. [Security Audit Results](#security-audit-results)

## Security Architecture

### Defense in Depth

The P2P File Converter implements multiple layers of security:

```
┌─────────────────────────────────────────────────────────┐
│                  Application Layer                      │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐      │
│  │Input        │ │File         │ │Resource     │      │
│  │Validation   │ │Validation   │ │Limits       │      │
│  └─────────────┘ └─────────────┘ └─────────────┘      │
├─────────────────────────────────────────────────────────┤
│                  Protocol Layer                        │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐      │
│  │Message      │ │Flow         │ │Timeout      │      │
│  │Validation   │ │Control      │ │Protection   │      │
│  └─────────────┘ └─────────────┘ └─────────────┘      │
├─────────────────────────────────────────────────────────┤
│                  Transport Layer                       │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐      │
│  │Noise        │ │Peer         │ │Connection   │      │
│  │Encryption   │ │Authentication│ │Limits       │      │
│  └─────────────┘ └─────────────┘ └─────────────┘      │
├─────────────────────────────────────────────────────────┤
│                  Network Layer                         │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐      │
│  │TCP          │ │Rate         │ │Firewall     │      │
│  │Security     │ │Limiting     │ │Integration  │      │
│  └─────────────┘ └─────────────┘ └─────────────┘      │
└─────────────────────────────────────────────────────────┘
```

### Security Principles

1. **Confidentiality**: All peer communication is encrypted
2. **Integrity**: Messages and files are protected against tampering
3. **Authentication**: Peer identities are cryptographically verified
4. **Authorization**: File transfer permissions are enforced
5. **Availability**: DoS protection and resource limits
6. **Non-repudiation**: Transfer logs provide accountability

## Threat Model

### Assets

- **User Files**: Documents being converted and transferred
- **System Resources**: CPU, memory, disk space, network bandwidth
- **Peer Identity**: Cryptographic keys and peer IDs
- **Configuration Data**: System settings and credentials
- **Transfer Logs**: Activity records and metadata

### Threats

#### Network Threats

| Threat | Impact | Likelihood | Mitigation |
|--------|--------|------------|------------|
| Man-in-the-middle | High | Medium | Noise protocol with peer verification |
| Eavesdropping | High | High | End-to-end encryption |
| Replay attacks | Medium | Low | Unique message IDs and timestamps |
| Connection hijacking | High | Low | Mutual authentication |
| DDoS attacks | Medium | Medium | Rate limiting and connection limits |

#### Application Threats

| Threat | Impact | Likelihood | Mitigation |
|--------|--------|------------|------------|
| Malicious files | High | Medium | File validation and sandboxing |
| Path traversal | High | Low | Path sanitization |
| Resource exhaustion | Medium | Medium | Resource limits and quotas |
| Protocol abuse | Medium | Low | Message validation |
| Information leakage | Medium | Low | Minimal error information |

#### System Threats

| Threat | Impact | Likelihood | Mitigation |
|--------|--------|------------|------------|
| Privilege escalation | High | Low | Minimal privileges |
| File system access | High | Low | Restricted file paths |
| Memory corruption | High | Very Low | Rust memory safety |
| Configuration tampering | Medium | Low | Configuration validation |

### Attack Scenarios

#### Scenario 1: Malicious File Transfer

**Attack**: Adversary sends malicious file designed to exploit conversion process.

**Mitigations**:
- File type validation before processing
- Size limits prevent resource exhaustion
- Sandboxed conversion environment
- Input sanitization for file content

#### Scenario 2: Network-Based Attack

**Attack**: Adversary intercepts and modifies network traffic.

**Mitigations**:
- Noise protocol provides authenticated encryption
- Peer identity verification prevents impersonation
- Message integrity verification detects tampering
- Connection timeout prevents indefinite blocking

#### Scenario 3: Resource Exhaustion

**Attack**: Adversary floods system with requests or large files.

**Mitigations**:
- Connection rate limiting
- File size limits
- Memory usage monitoring
- Concurrent transfer limits

## Transport Security

### Noise Protocol Implementation

The system uses the Noise protocol (XX handshake pattern) for secure transport:

```rust
// Noise configuration
let params = NoiseParams::new(
    "Noise_XX_25519_ChaChaPoly_BLAKE2s".parse().unwrap()
);

// Key generation
let private_key = x25519_dalek::StaticSecret::new(&mut OsRng);
let public_key = x25519_dalek::PublicKey::from(&private_key);
```

#### Handshake Process

```
Initiator (I)                    Responder (R)
---------                        ---------

Phase 1: Initial exchange
I -> R: e (ephemeral public key)
R -> I: e, ee (ephemeral key + DH)

Phase 2: Authentication  
I -> R: s (static public key), se
R -> I: s, se

Phase 3: Completion
Both parties: Derive transport keys
```

#### Security Properties

- **Forward Secrecy**: Session keys deleted after use
- **Mutual Authentication**: Both peers verify identity
- **Perfect Forward Secrecy**: Compromise of long-term keys doesn't affect past sessions
- **Replay Protection**: Message counters prevent replay attacks

### Key Management

```rust
#[derive(Debug)]
pub struct PeerKeys {
    /// Long-term static key pair
    static_keypair: ed25519_dalek::Keypair,

    /// Current session keys (ephemeral)
    session_keys: Option<SessionKeys>,

    /// Known peer public keys
    known_peers: HashMap<PeerId, ed25519_dalek::PublicKey>,
}

impl PeerKeys {
    /// Generate new static keypair
    pub fn generate() -> Self {
        let mut csprng = OsRng{};
        let static_keypair = ed25519_dalek::Keypair::generate(&mut csprng);

        Self {
            static_keypair,
            session_keys: None,
            known_peers: HashMap::new(),
        }
    }

    /// Add trusted peer
    pub fn add_trusted_peer(&mut self, peer_id: PeerId, public_key: ed25519_dalek::PublicKey) {
        self.known_peers.insert(peer_id, public_key);
    }

    /// Verify peer signature
    pub fn verify_peer(&self, peer_id: &PeerId, message: &[u8], signature: &[u8]) -> bool {
        if let Some(public_key) = self.known_peers.get(peer_id) {
            public_key.verify_strict(message, &signature.try_into().unwrap_or_default()).is_ok()
        } else {
            false
        }
    }
}
```

## Application Security

### File Validation Pipeline

```rust
pub struct FileValidator {
    max_file_size: u64,
    allowed_types: HashSet<String>,
    scan_enabled: bool,
}

impl FileValidator {
    pub async fn validate_file(&self, file_path: &Path) -> Result<ValidationResult> {
        let mut result = ValidationResult::default();

        // 1. Basic file system checks
        self.validate_file_system(file_path, &mut result).await?;

        // 2. File size validation
        self.validate_file_size(file_path, &mut result).await?;

        // 3. File type validation
        self.validate_file_type(file_path, &mut result).await?;

        // 4. Content scanning (if enabled)
        if self.scan_enabled {
            self.scan_file_content(file_path, &mut result).await?;
        }

        Ok(result)
    }

    async fn validate_file_system(&self, path: &Path, result: &mut ValidationResult) -> Result<()> {
        // Check for path traversal
        let canonical = path.canonicalize()
            .context("Failed to canonicalize path")?;

        if canonical.components().any(|c| matches!(c, Component::ParentDir)) {
            result.add_error("Path traversal detected");
            return Err(SecurityError::PathTraversal.into());
        }

        // Check file permissions
        let metadata = fs::metadata(path)
            .context("Failed to read file metadata")?;

        if !metadata.is_file() {
            result.add_error("Path is not a regular file");
            return Err(SecurityError::InvalidFileType.into());
        }

        Ok(())
    }

    async fn scan_file_content(&self, path: &Path, result: &mut ValidationResult) -> Result<()> {
        let mut file = fs::File::open(path).await?;
        let mut buffer = vec![0u8; 8192]; // Read first 8KB

        let bytes_read = file.read(&mut buffer).await?;
        buffer.truncate(bytes_read);

        // Check for suspicious patterns
        if self.contains_suspicious_patterns(&buffer) {
            result.add_warning("File contains potentially suspicious content");
        }

        // Check for executable signatures
        if self.is_executable_content(&buffer) {
            result.add_error("Executable content detected");
            return Err(SecurityError::ExecutableContent.into());
        }

        Ok(())
    }

    fn contains_suspicious_patterns(&self, data: &[u8]) -> bool {
        // Common malware signatures
        let suspicious_patterns = [
            b"EICAR-STANDARD-ANTIVIRUS-TEST-FILE",
            b"\x4d\x5a", // PE header
            b"\x7f\x45\x4c\x46", // ELF header
        ];

        suspicious_patterns.iter().any(|pattern| {
            data.windows(pattern.len()).any(|window| window == *pattern)
        })
    }
}
```

### Sandboxed File Processing

```rust
pub struct SandboxedConverter {
    temp_dir: TempDir,
    resource_limits: ResourceLimits,
}

impl SandboxedConverter {
    pub async fn convert_file(&self, input: &Path, output: &Path) -> Result<()> {
        // Create isolated temporary directory
        let sandbox_dir = self.temp_dir.path().join("sandbox");
        fs::create_dir_all(&sandbox_dir).await?;

        // Copy input file to sandbox
        let sandbox_input = sandbox_dir.join("input");
        fs::copy(input, &sandbox_input).await?;

        // Set resource limits
        self.apply_resource_limits()?;

        // Perform conversion in isolated environment
        let result = self.perform_conversion(&sandbox_input, &sandbox_dir).await;

        // Clean up sandbox
        let _ = fs::remove_dir_all(&sandbox_dir).await;

        result
    }

    fn apply_resource_limits(&self) -> Result<()> {
        // Set memory limit
        #[cfg(unix)]
        {
            use libc::{setrlimit, rlimit, RLIMIT_AS};

            let limit = rlimit {
                rlim_cur: self.resource_limits.max_memory,
                rlim_max: self.resource_limits.max_memory,
            };

            unsafe {
                if setrlimit(RLIMIT_AS, &limit) != 0 {
                    return Err(SecurityError::ResourceLimitFailed.into());
                }
            }
        }

        Ok(())
    }
}
```

## Input Validation

### Protocol Message Validation

```rust
pub struct MessageValidator {
    max_message_size: usize,
    allowed_versions: HashSet<String>,
}

impl MessageValidator {
    pub fn validate_transfer_request(&self, request: &FileTransferRequest) -> Result<()> {
        // Validate transfer ID format
        if !self.is_valid_uuid(&request.transfer_id) {
            return Err(ValidationError::InvalidTransferId.into());
        }

        // Validate filename
        self.validate_filename(&request.filename)?;

        // Validate file size
        if request.file_size > MAX_FILE_SIZE {
            return Err(ValidationError::FileTooLarge.into());
        }

        // Validate file type
        if !SUPPORTED_FILE_TYPES.contains(&request.file_type) {
            return Err(ValidationError::UnsupportedFileType.into());
        }

        // Validate metadata
        self.validate_metadata(&request.metadata)?;

        Ok(())
    }

    fn validate_filename(&self, filename: &str) -> Result<()> {
        // Length check
        if filename.len() > 255 {
            return Err(ValidationError::FilenameTooLong.into());
        }

        // Character validation
        if filename.chars().any(|c| matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\0')) {
            return Err(ValidationError::InvalidFilename.into());
        }

        // Path traversal check
        if filename.contains("..") || filename.contains('/') || filename.contains('\') {
            return Err(ValidationError::PathTraversal.into());
        }

        Ok(())
    }

    fn validate_metadata(&self, metadata: &HashMap<String, String>) -> Result<()> {
        // Limit metadata size
        let total_size: usize = metadata.iter()
            .map(|(k, v)| k.len() + v.len())
            .sum();

        if total_size > MAX_METADATA_SIZE {
            return Err(ValidationError::MetadataTooLarge.into());
        }

        // Validate metadata keys and values
        for (key, value) in metadata {
            if key.len() > MAX_METADATA_KEY_LENGTH {
                return Err(ValidationError::MetadataKeyTooLong.into());
            }

            if value.len() > MAX_METADATA_VALUE_LENGTH {
                return Err(ValidationError::MetadataValueTooLong.into());
            }

            // Check for potentially dangerous metadata
            if DANGEROUS_METADATA_KEYS.contains(key) {
                return Err(ValidationError::DangerousMetadata.into());
            }
        }

        Ok(())
    }
}
```

## Resource Protection

### Rate Limiting

```rust
pub struct RateLimiter {
    buckets: Arc<RwLock<HashMap<IpAddr, TokenBucket>>>,
    global_bucket: Arc<RwLock<TokenBucket>>,
}

impl RateLimiter {
    pub async fn check_rate_limit(&self, peer_addr: IpAddr) -> Result<()> {
        // Check global rate limit
        {
            let mut global = self.global_bucket.write().await;
            global.refill();
            if !global.try_consume(1) {
                return Err(SecurityError::GlobalRateLimitExceeded.into());
            }
        }

        // Check per-peer rate limit
        {
            let mut buckets = self.buckets.write().await;
            let bucket = buckets.entry(peer_addr)
                .or_insert_with(|| TokenBucket::new(10, Duration::from_secs(60)));

            bucket.refill();
            if !bucket.try_consume(1) {
                return Err(SecurityError::PeerRateLimitExceeded.into());
            }
        }

        Ok(())
    }
}

pub struct TokenBucket {
    capacity: u32,
    tokens: u32,
    last_refill: Instant,
    refill_rate: Duration,
}

impl TokenBucket {
    pub fn new(capacity: u32, refill_period: Duration) -> Self {
        Self {
            capacity,
            tokens: capacity,
            last_refill: Instant::now(),
            refill_rate: refill_period,
        }
    }

    pub fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);

        if elapsed >= self.refill_rate {
            self.tokens = self.capacity;
            self.last_refill = now;
        }
    }

    pub fn try_consume(&mut self, tokens: u32) -> bool {
        if self.tokens >= tokens {
            self.tokens -= tokens;
            true
        } else {
            false
        }
    }
}
```

### Memory Protection

```rust
pub struct MemoryMonitor {
    max_memory: u64,
    current_usage: Arc<AtomicU64>,
    allocations: Arc<RwLock<HashMap<String, u64>>>,
}

impl MemoryMonitor {
    pub fn check_allocation(&self, size: u64, operation: &str) -> Result<MemoryGuard> {
        let current = self.current_usage.load(Ordering::SeqCst);

        if current + size > self.max_memory {
            return Err(SecurityError::MemoryLimitExceeded.into());
        }

        // Track allocation
        self.current_usage.fetch_add(size, Ordering::SeqCst);
        {
            let mut allocations = self.allocations.write().unwrap();
            *allocations.entry(operation.to_string()).or_insert(0) += size;
        }

        Ok(MemoryGuard {
            monitor: self,
            size,
            operation: operation.to_string(),
        })
    }
}

pub struct MemoryGuard<'a> {
    monitor: &'a MemoryMonitor,
    size: u64,
    operation: String,
}

impl<'a> Drop for MemoryGuard<'a> {
    fn drop(&mut self) {
        self.monitor.current_usage.fetch_sub(self.size, Ordering::SeqCst);

        let mut allocations = self.monitor.allocations.write().unwrap();
        if let Some(current) = allocations.get_mut(&self.operation) {
            *current = current.saturating_sub(self.size);
            if *current == 0 {
                allocations.remove(&self.operation);
            }
        }
    }
}
```

## Security Testing

### Fuzzing

```rust
#[cfg(test)]
mod fuzz_tests {
    use super::*;
    use arbitrary::{Arbitrary, Unstructured};

    #[derive(Arbitrary, Debug)]
    struct FuzzInput {
        filename: String,
        file_size: u64,
        file_type: String,
        metadata: Vec<(String, String)>,
    }

    #[test]
    fn fuzz_message_validation() {
        bolero::check!()
            .with_type::<FuzzInput>()
            .for_each(|input| {
                let request = FileTransferRequest {
                    transfer_id: "test-id".to_string(),
                    filename: input.filename.clone(),
                    file_size: input.file_size,
                    file_type: input.file_type.clone(),
                    target_format: None,
                    return_result: false,
                    chunk_count: 1,
                    metadata: input.metadata.iter().cloned().collect(),
                };

                let validator = MessageValidator::new();
                let _ = validator.validate_transfer_request(&request);
                // Should not panic regardless of input
            });
    }
}
```

### Penetration Testing

```rust
#[cfg(test)]
mod security_tests {
    use super::*;

    #[tokio::test]
    async fn test_path_traversal_protection() {
        let validator = FileValidator::new();

        let malicious_paths = vec![
            "../../../etc/passwd",
            "..\windows\system32\config\sam",
            "/etc/shadow",
            "../../../../proc/version",
        ];

        for path in malicious_paths {
            let result = validator.validate_filename(path);
            assert!(result.is_err(), "Path traversal should be blocked: {}", path);
        }
    }

    #[tokio::test]
    async fn test_resource_exhaustion_protection() {
        let monitor = MemoryMonitor::new(1024 * 1024); // 1MB limit

        // Try to allocate more than limit
        let result = monitor.check_allocation(2 * 1024 * 1024, "test");
        assert!(result.is_err());

        // Small allocations should work
        let guard = monitor.check_allocation(512 * 1024, "test").unwrap();

        // Another large allocation should fail
        let result2 = monitor.check_allocation(1024 * 1024, "test2");
        assert!(result2.is_err());

        // After dropping guard, allocation should work
        drop(guard);
        let result3 = monitor.check_allocation(512 * 1024, "test3");
        assert!(result3.is_ok());
    }
}
```

## Incident Response

### Security Event Logging

```rust
pub struct SecurityLogger {
    log_file: Arc<Mutex<File>>,
    alert_threshold: SecurityLevel,
}

impl SecurityLogger {
    pub async fn log_security_event(&self, event: SecurityEvent) {
        let log_entry = LogEntry {
            timestamp: Utc::now(),
            event_type: event.event_type(),
            severity: event.severity(),
            peer_id: event.peer_id(),
            description: event.description(),
            metadata: event.metadata(),
        };

        // Write to log file
        let mut file = self.log_file.lock().await;
        let json_entry = serde_json::to_string(&log_entry).unwrap();
        writeln!(file, "{}", json_entry).await.unwrap();

        // Check if this requires immediate attention
        if log_entry.severity >= self.alert_threshold {
            self.send_alert(log_entry).await;
        }
    }

    async fn send_alert(&self, entry: LogEntry) {
        // Send to monitoring system, email, etc.
        eprintln!("SECURITY ALERT: {}", entry.description);
    }
}

#[derive(Debug)]
pub enum SecurityEvent {
    AuthenticationFailure { peer_id: PeerId, reason: String },
    RateLimitExceeded { peer_addr: IpAddr, requests: u32 },
    MaliciousFileDetected { filename: String, threat_type: String },
    UnauthorizedAccess { peer_id: PeerId, resource: String },
    ResourceExhaustionAttempt { peer_id: PeerId, resource_type: String },
}
```

### Automated Response

```rust
pub struct SecurityResponseSystem {
    blocked_peers: Arc<RwLock<HashMap<PeerId, BlockInfo>>>,
    blocked_ips: Arc<RwLock<HashMap<IpAddr, BlockInfo>>>,
}

impl SecurityResponseSystem {
    pub async fn handle_security_event(&self, event: SecurityEvent) {
        match event {
            SecurityEvent::RateLimitExceeded { peer_addr, .. } => {
                self.temporarily_block_ip(peer_addr, Duration::from_secs(300)).await;
            }
            SecurityEvent::MaliciousFileDetected { .. } => {
                // Block peer for longer duration
                if let Some(peer_id) = event.peer_id() {
                    self.block_peer(peer_id, Duration::from_secs(3600)).await;
                }
            }
            SecurityEvent::AuthenticationFailure { peer_id, .. } => {
                self.increment_failure_count(peer_id).await;
            }
            _ => {}
        }
    }

    async fn block_peer(&self, peer_id: PeerId, duration: Duration) {
        let block_info = BlockInfo {
            blocked_at: Instant::now(),
            duration,
            reason: "Security policy violation".to_string(),
        };

        self.blocked_peers.write().await.insert(peer_id, block_info);
    }

    pub async fn is_peer_blocked(&self, peer_id: &PeerId) -> bool {
        let blocked_peers = self.blocked_peers.read().await;

        if let Some(block_info) = blocked_peers.get(peer_id) {
            block_info.blocked_at.elapsed() < block_info.duration
        } else {
            false
        }
    }
}
```

This security documentation provides comprehensive coverage of the security measures implemented in the P2P File Converter, including threat analysis, mitigation strategies, and incident response procedures.
