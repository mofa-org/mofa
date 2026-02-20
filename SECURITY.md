# Security Policy

## Supported Versions

Security updates are provided for the latest stable release of MoFA. For information about currently supported versions, please see the [release notes](https://github.com/mofa-org/mofa/releases).

| Version | Supported          |
| ------- | ------------------ |
| latest  | Yes |
| other   | No |

## Reporting a Vulnerability

The MoFA team takes security vulnerabilities seriously. We appreciate your efforts to responsibly disclose your findings.

If you discover a security vulnerability, please report it to us privately before disclosing it publicly.

### How to Report

**Preferred Method**: Use GitHub's private vulnerability reporting feature:
- Visit https://github.com/mofa-org/mofa/security/advisories
- Click "Report a vulnerability"
- Follow the prompts to submit your report privately

**Alternative**: Send an email to the security team at security@mofa.ai

### What to Include

Please include as much of the following information in your report as possible:

- A description of the vulnerability
- Steps to reproduce the vulnerability
- Proof-of-concept or exploit code (if available)
- Potential impact of the vulnerability
- Affected versions (if known)
- Suggested mitigation (if any)

### Response Timeline

We will acknowledge receipt of your vulnerability report within 3 business days and provide a detailed response within 14 days regarding the next steps. You can expect:

- Initial confirmation of receipt within 3 business days
- Initial assessment of the report within 7 business days
- Detailed response and remediation timeline within 14 business days

### Disclosure Policy

Once a vulnerability is reported:

1. We will investigate and confirm the vulnerability
2. We will develop a fix and coordinate a release date
3. We will publish a security advisory with details about the vulnerability, affected versions, and remediation steps
4. You will be credited for the discovery (unless you prefer to remain anonymous)

We ask that you do not disclose the vulnerability publicly until we have released a fix and published a security advisory, unless:

- You have attempted to contact us and received no response after 30 days
- We have agreed to a disclosure timeline that has expired
- The vulnerability is being actively exploited in the wild

## Security Best Practices

### For Users

- Keep MoFA updated to the latest version
- Never commit API keys, credentials, or secrets to version control
- Use environment variables or secure secret management systems for sensitive data
- Review the [comprehensive security guide](docs/security.md) for detailed recommendations
- Enable Rhai scripting sandbox limits in production environments
- Validate all user-provided scripts before execution
- Use WASM sandboxing for untrusted plugins when available

### For Contributors

- Follow secure coding practices outlined in our [contributing guide](CONTRIBUTING.md)
- Never commit secrets, credentials, or sensitive data
- Use environment variables for configuration secrets
- Review changes for security implications
- Report security vulnerabilities through the proper channels (not in public issues)

## Security Features

MoFA includes several security-focused features:

- **WASM Sandboxing**: Compile-time plugins can be isolated using WebAssembly
- **Rhai Script Limits**: Configurable resource limits for runtime scripts (memory, CPU, operations)
- **Type Safety**: Rust's memory safety and type system prevent entire classes of vulnerabilities
- **Plugin Verification**: Plugin metadata includes version information and integrity checks

## Known Security Considerations

### Runtime Scripting (Rhai)

The Rhai scripting engine provides powerful runtime programmability but requires careful configuration:

- Always set resource limits (max operations, memory, execution time)
- Scripts run with access to registered functions and APIs
- Hot-reloading scripts in production requires careful validation
- Review the [security documentation](docs/security.md) for detailed configuration guidance

### Plugin System

Plugins have access to agent capabilities and system resources:

- Only install plugins from trusted sources
- Review plugin code before deployment in production
- Use WASM plugins for untrusted code when available
- Keep plugins updated to receive security fixes

### Credential Management

Multiple LLM providers require API keys and credentials:

- Use environment variables for credentials (recommended)
- Rotate credentials regularly
- Never include credentials in logs or error messages
- Use different credentials for development and production

### Distributed Deployments

When using MoFA's distributed capabilities:

- Secure communication channels between agents
- Implement authentication and authorization for multi-agent systems
- Enable audit logging for security-sensitive operations
- Monitor for suspicious activity

## Receiving Security Updates

To stay informed about security updates:

- Watch the [MoFA repository](https://github.com/mofa-org/mofa) on GitHub
- Subscribe to [security advisories](https://github.com/mofa-org/mofa/security/advisories)
- Follow [@mofa_ai](https://twitter.com/mofa_ai) on Twitter for announcements
- Join the [MoFA Discord](https://discord.com/invite/hKJZzDMMm9) for community discussions

## Security Audits

If your organization requires a formal security audit or has specific security compliance requirements, please contact us at security@mofa.ai.

## License

By reporting a security vulnerability, you agree that your report will be used to improve the security of MoFA for all users. We will credit you for your contribution unless you request anonymity.

Thank you for helping keep MoFA and our users safe!
